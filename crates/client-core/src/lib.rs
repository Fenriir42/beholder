//! Beholder client core: connection manager, outbox, local cache and sync.
//! No Tauri dependency, so it is testable headless.

pub mod cache;
pub mod connection;
pub mod http;
pub mod view;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};

use beholder_core::{CampaignId, CommandId, UserId};
use beholder_protocol::{Command, CommandKind, RejectReason, ServerMsg, SessionToken};
use tokio::sync::mpsc;

use crate::cache::{Cache, CacheError};
use crate::connection::Wake;
use crate::view::View;

/// What the UI shows in the connection pill (SPEC §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    /// First connection attempt of this run.
    Connecting,
    /// Connected and synced.
    Online,
    /// Lost the connection; retrying. `queued` commands are waiting.
    Reconnecting {
        /// Commands in the outbox.
        queued: usize,
    },
    /// Several attempts failed; showing cached data, still retrying.
    Offline {
        /// Commands in the outbox.
        queued: usize,
    },
}

/// Notifications for the shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientEvent {
    /// The connection state changed.
    State(ConnectionState),
    /// Cached or pending data changed; re-read [`Client::view`].
    ViewChanged,
    /// The server refused a command; it has been rolled back.
    Rejected {
        /// The command.
        id: CommandId,
        /// Why.
        reason: RejectReason,
    },
    /// Store this token in place of the old one.
    SessionRotated(SessionToken),
    /// The token is no longer valid; the user must log in again.
    SessionInvalid,
    /// The server speaks a newer protocol; the app must be updated.
    UpdateRequired {
        /// The server's protocol version.
        server_version: u16,
    },
}

/// State shared by the handle and the manager task.
pub(crate) struct Shared {
    cache: Mutex<Cache>,
    presence: Mutex<HashMap<CampaignId, HashSet<UserId>>>,
    state: Mutex<ConnectionState>,
    events: mpsc::UnboundedSender<ClientEvent>,
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
        // The shell may have gone away during shutdown; nothing to do then.
        let _ = self.events.send(e);
    }

    pub(crate) fn clear_presence(&self) {
        lock(&self.presence).clear();
    }

    /// Applies one server message to the cache and presence.
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
            ServerMsg::Pong { .. } | ServerMsg::HelloOk(_) | ServerMsg::HelloErr(_) => {
                return Ok(());
            }
        }
        self.emit(ClientEvent::ViewChanged);
        Ok(())
    }
}

/// Handle to a running client. Cheap to clone.
#[derive(Clone)]
pub struct Client {
    shared: Arc<Shared>,
    wake: mpsc::UnboundedSender<Wake>,
}

impl Client {
    /// Starts the connection manager on the current tokio runtime. The cache
    /// is usable immediately, so the UI can render before any network I/O.
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
        });
        tokio::spawn(connection::run(
            Arc::clone(&shared),
            ws_url(server_url),
            token,
            wake_rx,
        ));
        (Self { shared, wake }, events_rx)
    }

    /// The current optimistic view.
    pub fn view(&self) -> Result<View, CacheError> {
        let (user, confirmed, outbox) = self
            .shared
            .with_cache(|c| Ok((c.user()?, c.campaigns()?, c.outbox()?)))?;
        let presence = lock(&self.shared.presence).clone();
        Ok(view::compute(user, confirmed, &outbox, &presence))
    }

    /// Records a user action: persisted in the outbox first, then sent when
    /// online. Returns its id, to match a later `Rejected`.
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

    /// The current connection state.
    pub fn state(&self) -> ConnectionState {
        *lock(&self.shared.state)
    }

    /// Retry now: the OS reported a network change, or the user asked.
    pub fn network_changed(&self) {
        let _ = self.wake.send(Wake::NetworkChanged);
    }

    /// Closes the connection and stops the manager.
    pub fn shutdown(&self) {
        let _ = self.wake.send(Wake::Shutdown);
    }
}

/// The WebSocket URL for a server base URL (`https://host` → `wss://host/ws`).
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
