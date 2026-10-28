pub mod cache;
pub mod compendium;
pub mod connection;
pub mod http;
pub mod library_file;
pub mod view;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};

use beholder_core::compendium::{
    Alternative, Entry, FacetOptions, Hit, LicenseText, SearchQuery, SourceMeta,
};
use beholder_core::compendium::{Kind, RulesVersion};
use beholder_core::library::{ImportSummary, LibraryDraft, LibraryOp, MAX_BATCH};
use beholder_core::{CampaignId, CommandId, LibraryId, UserId};
use beholder_protocol::{Command, CommandKind, RejectReason, ServerMsg, SessionToken};
use library_file::{FileError, FileFormat};
use tokio::sync::mpsc;

use crate::cache::{Cache, CacheError};
use crate::connection::Wake;
use crate::view::View;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Online,
    Reconnecting { queued: usize },
    Offline { queued: usize },
}

const BATCH_BYTES: usize = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("cache: {0}")]
    Cache(#[from] CacheError),
    #[error("file: {0}")]
    File(#[from] FileError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompendiumState {
    Syncing,
    Ready,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientEvent {
    Compendium(CompendiumState),
    State(ConnectionState),
    ViewChanged,
    Rejected { id: CommandId, reason: RejectReason },
    SessionRotated(SessionToken),
    SessionInvalid,
    UpdateRequired { server_version: u16 },
}

pub(crate) struct Shared {
    cache: Mutex<Cache>,
    presence: Mutex<HashMap<CampaignId, HashSet<UserId>>>,
    state: Mutex<ConnectionState>,
    events: mpsc::UnboundedSender<ClientEvent>,
    server_url: String,
    compendium_busy: AtomicBool,
    compendium_synced: Mutex<Option<Vec<String>>>,
    library_dirty: AtomicBool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Shared {
    pub(crate) fn with_cache<R>(
        &self,
        f: impl FnOnce(&Cache) -> Result<R, CacheError>,
    ) -> Result<R, CacheError> {
        f(&lock(&self.cache))
    }

    pub(crate) fn queued(&self) -> usize {
        self.with_cache(|c| c.outbox_len()).unwrap_or(0)
    }

    pub(crate) fn set_state(&self, s: ConnectionState) {
        let mut state = lock(&self.state);
        if *state != s {
            *state = s;
            drop(state);
            self.emit(ClientEvent::State(s));
        }
    }

    pub(crate) fn emit(&self, e: ClientEvent) {
        let _ = self.events.send(e);
    }

    pub(crate) fn clear_presence(&self) {
        lock(&self.presence).clear();
    }

    pub(crate) fn handle(&self, msg: ServerMsg) -> Result<(), CacheError> {
        match msg {
            ServerMsg::Ack { id, .. } => {
                self.with_cache(|c| c.remove_outbox(id))?;
            }
            ServerMsg::Reject { id, reason } => {
                self.with_cache(|c| c.remove_outbox(id))?;
                self.emit(ClientEvent::Rejected { id, reason });
            }
            ServerMsg::Events {
                campaign_id,
                events,
            } => {
                self.with_cache(|c| c.apply_events(campaign_id, &events))?;
            }
            ServerMsg::Snapshot { seq, campaign } => {
                self.with_cache(|c| c.put_snapshot(seq, &campaign))?;
            }
            ServerMsg::CampaignRemoved { campaign_id } => {
                self.with_cache(|c| c.remove_campaign(campaign_id))?;
            }
            ServerMsg::Presence {
                campaign_id,
                online,
            } => {
                lock(&self.presence).insert(campaign_id, online.into_iter().collect());
            }
            ServerMsg::PresenceChanged {
                campaign_id,
                user_id,
                online,
            } => {
                let mut p = lock(&self.presence);
                let set = p.entry(campaign_id).or_default();
                if online {
                    set.insert(user_id);
                } else {
                    set.remove(&user_id);
                }
            }
            ServerMsg::LibraryChanged { .. } => {
                self.library_dirty
                    .store(true, std::sync::atomic::Ordering::Release);
                return Ok(());
            }
            ServerMsg::Pong { .. } | ServerMsg::HelloOk(_) | ServerMsg::HelloErr(_) => {
                return Ok(());
            }
        }
        self.emit(ClientEvent::ViewChanged);
        Ok(())
    }
}

#[derive(Clone)]
pub struct Client {
    shared: Arc<Shared>,
    wake: mpsc::UnboundedSender<Wake>,
}

impl Client {
    pub fn start(
        server_url: &str,
        cache: Cache,
        token: SessionToken,
    ) -> (Self, mpsc::UnboundedReceiver<ClientEvent>) {
        let (events, events_rx) = mpsc::unbounded_channel();
        let (wake, wake_rx) = mpsc::unbounded_channel();
        let shared = Arc::new(Shared {
            cache: Mutex::new(cache),
            presence: Mutex::new(HashMap::new()),
            state: Mutex::new(ConnectionState::Connecting),
            events,
            server_url: server_url.to_owned(),
            compendium_busy: AtomicBool::new(false),
            compendium_synced: Mutex::new(None),
            library_dirty: AtomicBool::new(false),
        });
        tokio::spawn(connection::run(
            Arc::clone(&shared),
            ws_url(server_url),
            token,
            wake_rx,
        ));
        (Self { shared, wake }, events_rx)
    }

    pub fn view(&self) -> Result<View, CacheError> {
        let (user, confirmed, outbox) = self
            .shared
            .with_cache(|c| Ok((c.user()?, c.campaigns()?, c.outbox()?)))?;
        let presence = lock(&self.shared.presence).clone();
        Ok(view::compute(user, confirmed, &outbox, &presence))
    }

    pub fn submit(&self, kind: CommandKind) -> Result<CommandId, CacheError> {
        let cmd = Command {
            id: CommandId(uuid::Uuid::now_v7()),
            kind,
        };
        self.shared.with_cache(|c| c.push_outbox(&cmd))?;
        let id = cmd.id;
        let _ = self.wake.send(Wake::Send(cmd));
        self.shared.emit(ClientEvent::ViewChanged);
        Ok(id)
    }

    pub fn search(&self, q: &SearchQuery, limit: usize) -> Result<Vec<Hit>, CacheError> {
        self.shared.with_cache(|c| c.search(q, limit))
    }

    pub fn compendium_entry(
        &self,
        source: &str,
        key: &str,
    ) -> Result<Option<(Entry, SourceMeta)>, CacheError> {
        self.shared.with_cache(|c| c.compendium_entry(source, key))
    }

    pub fn alternatives(
        &self,
        source: &str,
        key: &str,
        allowed: &[String],
    ) -> Result<Vec<Alternative>, CacheError> {
        self.shared
            .with_cache(|c| c.alternatives(source, key, allowed))
    }

    pub fn compendium_licenses(&self) -> Result<Vec<LicenseText>, CacheError> {
        self.shared.with_cache(|c| c.compendium_licenses())
    }

    pub fn compendium_sources(&self) -> Result<Vec<SourceMeta>, CacheError> {
        self.shared.with_cache(|c| c.compendium_sources())
    }

    pub fn facet_options(&self) -> Result<FacetOptions, CacheError> {
        self.shared.with_cache(|c| c.facet_options())
    }
    pub fn library_save(&self, draft: LibraryDraft, base_version: u32) -> Result<(), CacheError> {
        self.submit(CommandKind::Library {
            op: LibraryOp::Save {
                draft,
                base_version,
            },
        })
        .map(|_| ())
    }

    pub fn library_delete(&self, id: LibraryId) -> Result<(), CacheError> {
        self.submit(CommandKind::Library {
            op: LibraryOp::Delete { id },
        })
        .map(|_| ())
    }

    pub fn library_export(&self, format: FileFormat) -> Result<String, ClientError> {
        let entries = self.shared.with_cache(|c| c.library_entries())?;
        Ok(library_file::export(&entries, format)?)
    }
    pub fn library_import(
        &self,
        bytes: &[u8],
        format: FileFormat,
        replace: bool,
    ) -> Result<ImportSummary, ClientError> {
        let text = std::str::from_utf8(bytes).map_err(|_| FileError::NotLibrary)?;
        let file = library_file::read(text, format)?;
        let (existing, me, campaigns) = self
            .shared
            .with_cache(|c| Ok((c.library_entries()?, c.user()?, c.campaigns()?)))?;
        let me = me.map(|u| u.id);
        let dm_anywhere = me.is_some_and(|m| {
            campaigns
                .iter()
                .any(|(_, c)| c.role_of(m) == Some(beholder_core::Role::Dm))
        });
        let key = |kind: Kind, name: &str, book: Option<&str>, rules: Option<RulesVersion>| {
            (
                kind,
                name.to_lowercase(),
                book.map(str::to_lowercase),
                rules,
            )
        };
        let mut known: HashMap<_, Option<(LibraryId, u32, bool)>> = existing
            .iter()
            .map(|e| {
                let target = e
                    .key
                    .parse()
                    .ok()
                    .map(LibraryId)
                    .zip(e.library)
                    .map(|(id, m)| (id, m.version, dm_anywhere || Some(m.author) == me));
                (
                    key(e.facets.kind(), &e.name, e.book.as_deref(), e.rules),
                    target,
                )
            })
            .collect();
        let mut summary = ImportSummary::default();
        let mut batch: Vec<(LibraryDraft, u32)> = Vec::new();
        let mut batch_bytes = 0;
        for (i, f) in file.iter().enumerate() {
            let p = match library_file::parse_entry(f) {
                Ok(p) => p,
                Err(reason) => {
                    summary
                        .invalid
                        .push(format!("#{} {}: {reason}", i + 1, f.name));
                    continue;
                }
            };
            let k = key(
                p.facets.kind(),
                p.name.as_str(),
                p.book.as_ref().map(|b| b.as_str()),
                Some(p.rules),
            );
            let (id, base) = match known.get(&k) {
                None => (LibraryId(uuid::Uuid::now_v7()), 0),
                Some(Some((id, version, true))) if replace => (*id, *version),
                Some(Some((_, _, false))) if replace => {
                    summary.not_yours += 1;
                    continue;
                }
                Some(_) => {
                    summary.skipped += 1;
                    continue;
                }
            };
            known.insert(k, None);
            let draft = LibraryDraft {
                id,
                name: p.name,
                facets: p.facets,
                text: p.text,
                book: p.book,
                rules: p.rules,
            };
            if let Err(e) = draft.check_facets() {
                summary.invalid.push(format!("#{} {}: {e}", i + 1, f.name));
                continue;
            }
            let size = draft.text.as_str().len() + draft.name.as_str().len() + 256;
            if !batch.is_empty() && (batch.len() == MAX_BATCH || batch_bytes + size > BATCH_BYTES) {
                self.submit_batch(std::mem::take(&mut batch))?;
                batch_bytes = 0;
            }
            batch.push((draft, base));
            batch_bytes += size;
            if base == 0 {
                summary.added += 1;
            } else {
                summary.replaced += 1;
            }
        }
        if !batch.is_empty() {
            self.submit_batch(batch)?;
        }
        Ok(summary)
    }

    fn submit_batch(&self, items: Vec<(LibraryDraft, u32)>) -> Result<(), CacheError> {
        self.submit(CommandKind::Library {
            op: LibraryOp::UpsertMany { items },
        })
        .map(|_| ())
    }

    pub fn state(&self) -> ConnectionState {
        *lock(&self.shared.state)
    }

    pub fn network_changed(&self) {
        let _ = self.wake.send(Wake::NetworkChanged);
    }

    pub fn shutdown(&self) {
        let _ = self.wake.send(Wake::Shutdown);
    }
}

pub fn ws_url(server_url: &str) -> String {
    let base = server_url.trim_end_matches('/');
    let rest = base
        .strip_prefix("https://")
        .map(|r| format!("wss://{r}"))
        .or_else(|| base.strip_prefix("http://").map(|r| format!("ws://{r}")))
        .unwrap_or_else(|| format!("wss://{base}"));
    format!("{rest}/ws")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_url_from_server_url() {
        assert_eq!(
            ws_url("https://dnd.example.org/"),
            "wss://dnd.example.org/ws"
        );
        assert_eq!(ws_url("http://127.0.0.1:8080"), "ws://127.0.0.1:8080/ws");
        assert_eq!(ws_url("dnd.example.org"), "wss://dnd.example.org/ws");
    }
}
