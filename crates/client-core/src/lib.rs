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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Online,
    Reconnecting { queued: usize },
    Offline { queued: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientEvent {
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
