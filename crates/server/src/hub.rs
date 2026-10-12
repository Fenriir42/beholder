//! Live connections: subscriptions, per-recipient fan-out and presence.
//!
//! Campaign data is published from inside `Db::call`, so frames enter each
//! connection's queue in commit order. Queues are bounded: a client that can't
//! keep up is disconnected and catches up on reconnect, instead of growing
//! server memory.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use axum::body::Bytes;
use beholder_core::{Campaign, CampaignId, Role, Seq, UserId};
use beholder_protocol::{EventEnvelope, codec};
use tokio::sync::mpsc;

use crate::permissions::Outgoing;

/// Frames buffered per connection before it counts as too slow.
pub const QUEUE_FRAMES: usize = 256;

/// Identifies one WebSocket connection.
pub type ConnId = u64;

struct Conn {
    user: UserId,
    tx: mpsc::Sender<Bytes>,
    campaigns: HashMap<CampaignId, Role>,
}

#[derive(Default)]
struct Inner {
    next_id: ConnId,
    conns: HashMap<ConnId, Conn>,
    /// Live connections per user.
    open: HashMap<UserId, u32>,
    /// Users other members have been told are online.
    announced: HashSet<UserId>,
}

/// Registry of live connections.
#[derive(Default)]
pub struct Hub {
    inner: Mutex<Inner>,
}

impl Hub {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Registers a connection. Returns its id and whether the user must now be
    /// announced online (they weren't already).
    pub fn register(&self, user: UserId, tx: mpsc::Sender<Bytes>) -> (ConnId, bool) {
        let mut inner = self.lock();
        inner.next_id += 1;
        let id = inner.next_id;
        inner.conns.insert(
            id,
            Conn {
                user,
                tx,
                campaigns: HashMap::new(),
            },
        );
        *inner.open.entry(user).or_default() += 1;
        let announce = inner.announced.insert(user);
        (id, announce)
    }

    /// Removes a connection; called exactly once when its socket task ends.
    /// Returns `true` when it was the user's last one, so the caller can
    /// announce them offline after a grace period.
    pub fn unregister(&self, id: ConnId, user: UserId) -> bool {
        let mut inner = self.lock();
        inner.conns.remove(&id);
        let Some(n) = inner.open.get_mut(&user) else {
            return false;
        };
        *n -= 1;
        if *n == 0 {
            inner.open.remove(&user);
            return true;
        }
        false
    }

    /// After the grace period: `true` if `user` is still offline and must be
    /// announced so (each user is announced offline once).
    pub fn expire_presence(&self, user: UserId) -> bool {
        let mut inner = self.lock();
        !inner.open.contains_key(&user) && inner.announced.remove(&user)
    }

    /// Subscribes one connection to a campaign.
    pub fn subscribe(&self, id: ConnId, campaign: CampaignId, role: Role) {
        if let Some(c) = self.lock().conns.get_mut(&id) {
            c.campaigns.insert(campaign, role);
        }
    }

    /// Queues a message for one connection.
    pub fn send(&self, id: ConnId, out: &Outgoing) {
        let mut inner = self.lock();
        if let Some(frame) = encode(out) {
            push(&mut inner, id, frame);
        }
    }

    /// Fans new events out to every subscribed connection, filtered per role.
    pub fn publish_events(&self, campaign: CampaignId, events: &[EventEnvelope]) {
        let mut inner = self.lock();
        // At most one encoding per role, however many connections there are.
        let mut frames: HashMap<Role, Option<Bytes>> = HashMap::new();
        let targets: Vec<(ConnId, Role)> = subscribers(&inner, campaign).collect();
        for (id, role) in targets {
            let frame = frames.entry(role).or_insert_with(|| {
                Outgoing::events(role, campaign, events).and_then(|o| encode(&o))
            });
            if let Some(frame) = frame {
                push(&mut inner, id, frame.clone());
            }
        }
    }

    /// Sends a control message (presence, …) to every connection subscribed to `campaign`.
    pub fn broadcast(&self, campaign: CampaignId, out: &Outgoing) {
        let mut inner = self.lock();
        let Some(frame) = encode(out) else { return };
        let targets: Vec<ConnId> = subscribers(&inner, campaign).map(|(id, _)| id).collect();
        for id in targets {
            push(&mut inner, id, frame.clone());
        }
    }

    /// Subscribes every connection of `user` to a campaign they just joined
    /// or created, and sends each one the snapshot and who is online.
    pub fn add_member(&self, user: UserId, role: Role, seq: Seq, campaign: &Campaign) {
        let ids: Vec<ConnId> = {
            let mut inner = self.lock();
            inner
                .conns
                .iter_mut()
                .filter(|(_, c)| c.user == user)
                .map(|(id, c)| {
                    c.campaigns.insert(campaign.id, role);
                    *id
                })
                .collect()
        };
        let snapshot = Outgoing::snapshot(role, seq, campaign);
        let presence = self.presence(campaign);
        for id in ids {
            self.send(id, &snapshot);
            self.send(id, &presence);
        }
    }

    /// Who of `campaign`'s members is online.
    pub fn presence(&self, campaign: &Campaign) -> Outgoing {
        let inner = self.lock();
        let online = campaign
            .members
            .iter()
            .map(|m| m.user_id)
            .filter(|u| inner.announced.contains(u))
            .collect();
        Outgoing::presence(campaign.id, online)
    }

    /// Number of live connections (for tests and metrics).
    pub fn connection_count(&self) -> usize {
        self.lock().conns.len()
    }
}

fn subscribers(inner: &Inner, campaign: CampaignId) -> impl Iterator<Item = (ConnId, Role)> + '_ {
    inner
        .conns
        .iter()
        .filter_map(move |(id, c)| c.campaigns.get(&campaign).map(|r| (*id, *r)))
}

fn encode(out: &Outgoing) -> Option<Bytes> {
    match codec::encode(out.msg()) {
        Ok(f) => Some(Bytes::from(f)),
        Err(e) => {
            tracing::error!("encode failed: {e}");
            None
        }
    }
}

/// Queues a frame; drops the connection if its queue is full or closed.
fn push(inner: &mut Inner, id: ConnId, frame: Bytes) {
    let Some(conn) = inner.conns.get(&id) else {
        return;
    };
    if conn.tx.try_send(frame).is_err() {
        tracing::info!(conn = id, "dropping slow or closed connection");
        // Dropping the sender closes the queue; the socket task sees it, closes
        // the socket and calls `unregister`, which does the presence bookkeeping.
        inner.conns.remove(&id);
    }
}
