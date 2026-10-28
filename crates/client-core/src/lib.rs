pub mod cache;
pub mod compendium;
pub mod connection;
pub mod http;
pub mod library_file;
pub mod mapping;
pub mod view;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};

use beholder_core::compendium::{
    Alternative, Entry, FacetOptions, Hit, LicenseText, SearchQuery, SourceMeta,
};
use beholder_core::compendium::{Kind, RulesVersion};
use beholder_core::library::{ImportSummary, LibraryDraft, LibraryOp, MAX_BATCH};
use beholder_core::paste::{self, Pasted};
use beholder_core::{CampaignId, CommandId, ImportId, LibraryId, NoteBody, NoteTitle, UserId};
use beholder_protocol::http::ImportInfo;
use beholder_protocol::{Command, CommandKind, RejectReason, ServerMsg, SessionToken};
use library_file::{FileError, FileFormat, Parsed};
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

type TargetKey = (Kind, String, Option<String>, Option<RulesVersion>);

type Target = (LibraryId, u32, bool);

fn target_key(
    kind: Kind,
    name: &str,
    book: Option<&str>,
    rules: Option<RulesVersion>,
) -> TargetKey {
    (
        kind,
        name.to_lowercase(),
        book.map(str::to_lowercase),
        rules,
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PastePreview {
    pub entry: Pasted,
    pub duplicate: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("cache: {0}")]
    Cache(#[from] CacheError),
    #[error("file: {0}")]
    File(#[from] FileError),
    #[error("http: {0}")]
    Http(#[from] http::HttpError),
    #[error("mapping: {0}")]
    Mapping(#[from] mapping::MappingError),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BulkScan {
    pub total: usize,
    pub kinds: Vec<(Kind, usize)>,
    pub books: Vec<(String, usize)>,
    pub duplicates: usize,
    pub partial: usize,
    pub invalid: Vec<String>,
    pub invalid_count: usize,
}

fn open_json(path: &Path) -> Result<std::io::BufReader<std::fs::File>, ClientError> {
    Ok(std::io::BufReader::new(
        std::fs::File::open(path).map_err(mapping::MappingError::from)?,
    ))
}

pub fn bulk_probe(path: &Path) -> Result<(mapping::Probe, mapping::Profile), ClientError> {
    let probe = mapping::probe(open_json(path)?)?;
    let profile = mapping::suggest(&probe);
    Ok((probe, profile))
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
    token: SessionToken,
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
            token: token.clone(),
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
        let items = file.iter().enumerate().map(|(i, f)| {
            (
                format!("#{} {}", i + 1, f.name),
                library_file::parse_entry(f),
            )
        });
        self.upsert(items, replace)
    }

    pub fn library_paste(
        &self,
        text: &str,
        book: Option<&str>,
        rules: RulesVersion,
        default: Kind,
    ) -> Result<Vec<PastePreview>, ClientError> {
        let targets = self.library_targets()?;
        Ok(paste::parse(text, default)
            .into_iter()
            .map(|entry| {
                let duplicate = targets.contains_key(&target_key(
                    entry.facets.kind(),
                    &entry.name,
                    book,
                    Some(rules),
                ));
                PastePreview { entry, duplicate }
            })
            .collect())
    }

    pub fn library_accept(
        &self,
        entries: Vec<Pasted>,
        book: Option<&str>,
        rules: RulesVersion,
        replace: bool,
    ) -> Result<ImportSummary, ClientError> {
        let book = book.map(str::trim).filter(|b| !b.is_empty());
        let items = entries.into_iter().map(|p| {
            let label = p.name.clone();
            let parsed = (|| {
                Ok(Parsed {
                    name: NoteTitle::new(&p.name).map_err(|e| format!("name: {e}"))?,
                    facets: p.facets,
                    text: NoteBody::new(p.text).map_err(|e| format!("text: {e}"))?,
                    book: book
                        .map(NoteTitle::new)
                        .transpose()
                        .map_err(|e| format!("book: {e}"))?,
                    rules,
                })
            })();
            (label, parsed)
        });
        self.upsert(items, replace)
    }

    pub fn library_imports(&self) -> Result<Vec<ImportInfo>, ClientError> {
        Ok(http::Http::new(&self.shared.server_url).library_imports(&self.shared.token)?)
    }

    pub fn library_undo_import(&self, import: ImportId) -> Result<(), ClientError> {
        self.submit(CommandKind::Library {
            op: LibraryOp::UndoImport { import },
        })?;
        Ok(())
    }

    pub fn bulk_preview(
        &self,
        sample: &[serde_json::Value],
        profile: &mapping::Profile,
    ) -> Result<Vec<PastePreview>, ClientError> {
        let targets = self.library_targets()?;
        Ok(sample
            .iter()
            .filter_map(|v| mapping::map(profile, v).ok())
            .map(|m| {
                let p = m.parsed;
                let duplicate = targets.contains_key(&target_key(
                    p.facets.kind(),
                    p.name.as_str(),
                    p.book.as_ref().map(|b| b.as_str()),
                    Some(p.rules),
                ));
                PastePreview {
                    entry: Pasted {
                        name: p.name.as_str().to_owned(),
                        facets: p.facets,
                        text: p.text.as_str().to_owned(),
                        complete: !m.partial,
                    },
                    duplicate,
                }
            })
            .collect())
    }

    pub fn bulk_scan(
        &self,
        path: &Path,
        profile: &mapping::Profile,
    ) -> Result<BulkScan, ClientError> {
        let targets = self.library_targets()?;
        let mut scan = BulkScan::default();
        let mut kinds: std::collections::BTreeMap<Kind, usize> = Default::default();
        let mut books: HashMap<String, usize> = HashMap::new();
        let found = mapping::each(open_json(path)?, &profile.entries, |v| {
            scan.total += 1;
            match mapping::map(profile, &v) {
                Ok(m) => {
                    let p = &m.parsed;
                    *kinds.entry(p.facets.kind()).or_default() += 1;
                    let book = p
                        .book
                        .as_ref()
                        .map(|b| b.as_str().to_owned())
                        .unwrap_or_default();
                    *books.entry(book).or_default() += 1;
                    if targets.contains_key(&target_key(
                        p.facets.kind(),
                        p.name.as_str(),
                        p.book.as_ref().map(|b| b.as_str()),
                        Some(p.rules),
                    )) {
                        scan.duplicates += 1;
                    }
                    if m.partial {
                        scan.partial += 1;
                    }
                }
                Err(why) => {
                    scan.invalid_count += 1;
                    if scan.invalid.len() < 50 {
                        scan.invalid.push(format!("#{}: {why}", scan.total));
                    }
                }
            }
            true
        })?;
        if !found {
            return Err(mapping::MappingError::NoEntries.into());
        }
        scan.kinds = kinds.into_iter().collect();
        let mut books: Vec<(String, usize)> = books.into_iter().collect();
        books.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        books.truncate(20);
        scan.books = books;
        Ok(scan)
    }

    pub fn bulk_import(
        &self,
        path: &Path,
        profile: &mapping::Profile,
        replace: bool,
    ) -> Result<ImportSummary, ClientError> {
        let file = open_json(path)?;
        let (tx, rx) = std::sync::mpsc::sync_channel(512);
        let profile = profile.clone();
        let reader = std::thread::spawn(move || {
            let mut n = 0usize;
            mapping::each(file, &profile.entries, |v| {
                n += 1;
                let item = mapping::map(&profile, &v).map(|m| m.parsed);
                tx.send((format!("#{n}"), item)).is_ok()
            })
        });
        let mut summary = self.upsert(rx.into_iter(), replace)?;
        match reader.join() {
            Ok(Ok(true)) => {}
            Ok(Ok(false)) => summary
                .invalid
                .push(mapping::MappingError::NoEntries.to_string()),
            Ok(Err(e)) => summary.invalid.push(e.to_string()),
            Err(_) => summary.invalid.push("the file reader stopped".into()),
        }
        Ok(summary)
    }

    fn library_targets(&self) -> Result<HashMap<TargetKey, Option<Target>>, ClientError> {
        let (existing, me, campaigns) = self
            .shared
            .with_cache(|c| Ok((c.library_entries()?, c.user()?, c.campaigns()?)))?;
        let me = me.map(|u| u.id);
        let dm_anywhere = me.is_some_and(|m| {
            campaigns
                .iter()
                .any(|(_, c)| c.role_of(m) == Some(beholder_core::Role::Dm))
        });
        Ok(existing
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
                    target_key(e.facets.kind(), &e.name, e.book.as_deref(), e.rules),
                    target,
                )
            })
            .collect())
    }

    fn upsert(
        &self,
        items: impl Iterator<Item = (String, Result<Parsed, String>)>,
        replace: bool,
    ) -> Result<ImportSummary, ClientError> {
        let mut known = self.library_targets()?;
        let import = ImportId(uuid::Uuid::now_v7());
        let mut summary = ImportSummary::default();
        let mut batch: Vec<(LibraryDraft, u32)> = Vec::new();
        let mut batch_bytes = 0;
        for (label, parsed) in items {
            let p = match parsed {
                Ok(p) => p,
                Err(reason) => {
                    summary.invalid.push(format!("{label}: {reason}"));
                    continue;
                }
            };
            let k = target_key(
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
                summary.invalid.push(format!("{label}: {e}"));
                continue;
            }
            let size = draft.text.as_str().len() + draft.name.as_str().len() + 256;
            if !batch.is_empty() && (batch.len() == MAX_BATCH || batch_bytes + size > BATCH_BYTES) {
                self.submit_batch(import, std::mem::take(&mut batch))?;
                summary.import = Some(import);
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
            self.submit_batch(import, batch)?;
            summary.import = Some(import);
        }
        Ok(summary)
    }

    fn submit_batch(
        &self,
        import: ImportId,
        items: Vec<(LibraryDraft, u32)>,
    ) -> Result<(), CacheError> {
        self.submit(CommandKind::Library {
            op: LibraryOp::ImportMany { import, items },
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
