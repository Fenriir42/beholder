//! Local SQLite cache: confirmed campaign state, the outbox, and a few settings.
//!
//! It is a cache, not a source of truth: if its schema version doesn't match,
//! it is wiped and rebuilt from server snapshots (the outbox survives only
//! within one schema version, which is acceptable for a cache).

use std::path::Path;

use beholder_core::{Campaign, CampaignId, CommandId, Seq, User};
use beholder_protocol::{Command, EventEnvelope};
use rusqlite::{Connection, OptionalExtension, params};

/// Bumped when the tables below change; a mismatch wipes the cache.
const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE meta (key TEXT PRIMARY KEY, value BLOB NOT NULL) WITHOUT ROWID;
-- Confirmed state only: server events and snapshots, never optimistic changes.
CREATE TABLE campaigns (id BLOB PRIMARY KEY, last_seq INTEGER NOT NULL, state BLOB NOT NULL) WITHOUT ROWID;
-- Pending commands in submission order; deleted on Ack or Reject.
CREATE TABLE outbox (n INTEGER PRIMARY KEY AUTOINCREMENT, id BLOB NOT NULL UNIQUE, body BLOB NOT NULL);
";

/// A cache failure.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    /// SQLite failed.
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// A stored blob didn't decode.
    #[error("corrupt cache entry")]
    Corrupt,
}

impl From<postcard::Error> for CacheError {
    fn from(_: postcard::Error) -> Self {
        Self::Corrupt
    }
}

/// The local cache for one (server, user) pair.
pub struct Cache {
    conn: Connection,
}

impl Cache {
    /// Opens the cache file, creating or rebuilding it as needed.
    pub fn open(path: &Path) -> Result<Self, CacheError> {
        Self::init(Connection::open(path)?)
    }

    /// An in-memory cache, for tests.
    pub fn open_in_memory() -> Result<Self, CacheError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, CacheError> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA cache_size = -1024;",
        )?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version != SCHEMA_VERSION {
            conn.execute_batch(
                "DROP TABLE IF EXISTS meta; DROP TABLE IF EXISTS campaigns; DROP TABLE IF EXISTS outbox;",
            )?;
            conn.execute_batch(SCHEMA)?;
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        Ok(Self { conn })
    }

    /// Every cached campaign with the last seq applied to it.
    pub fn campaigns(&self) -> Result<Vec<(Seq, Campaign)>, CacheError> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT last_seq, state FROM campaigns")?;
        stmt.query_map([], |r| Ok((r.get::<_, u64>(0)?, r.get::<_, Vec<u8>>(1)?)))?
            .map(|row| {
                let (seq, state) = row?;
                Ok((Seq(seq), postcard::from_bytes(&state)?))
            })
            .collect()
    }

    /// `(campaign, last_seq)` for the `Hello` handshake.
    pub fn known(&self) -> Result<Vec<(CampaignId, Seq)>, CacheError> {
        Ok(self
            .campaigns()?
            .into_iter()
            .map(|(seq, c)| (c.id, seq))
            .collect())
    }

    fn campaign(&self, id: CampaignId) -> Result<Option<(Seq, Campaign)>, CacheError> {
        self.conn
            .query_row(
                "SELECT last_seq, state FROM campaigns WHERE id = ?1",
                [id.as_bytes()],
                |r| Ok((r.get::<_, u64>(0)?, r.get::<_, Vec<u8>>(1)?)),
            )
            .optional()?
            .map(|(seq, state)| Ok((Seq(seq), postcard::from_bytes(&state)?)))
            .transpose()
    }

    fn put(&self, seq: Seq, campaign: &Campaign) -> Result<(), CacheError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO campaigns (id, last_seq, state) VALUES (?1, ?2, ?3)",
            params![
                campaign.id.as_bytes(),
                seq.0,
                postcard::to_stdvec(campaign)?
            ],
        )?;
        Ok(())
    }

    /// Replaces a campaign with a server snapshot.
    pub fn put_snapshot(&self, seq: Seq, campaign: &Campaign) -> Result<(), CacheError> {
        self.put(seq, campaign)
    }

    /// Applies server events, skipping any already applied (seq ≤ last_seq).
    /// Returns `false` if the campaign isn't cached (a snapshot will follow).
    pub fn apply_events(
        &self,
        id: CampaignId,
        events: &[EventEnvelope],
    ) -> Result<bool, CacheError> {
        let Some((mut seq, mut campaign)) = self.campaign(id)? else {
            return Ok(false);
        };
        for e in events {
            if e.seq > seq {
                campaign.apply(&e.event);
                seq = e.seq;
            }
        }
        self.put(seq, &campaign)?;
        Ok(true)
    }

    /// Drops one campaign.
    pub fn remove_campaign(&self, id: CampaignId) -> Result<(), CacheError> {
        self.conn
            .execute("DELETE FROM campaigns WHERE id = ?1", [id.as_bytes()])?;
        Ok(())
    }

    /// Drops every campaign not in `keep` (the user's current memberships).
    pub fn retain_campaigns(&self, keep: &[CampaignId]) -> Result<(), CacheError> {
        for (_, c) in self.campaigns()? {
            if !keep.contains(&c.id) {
                self.remove_campaign(c.id)?;
            }
        }
        Ok(())
    }

    /// Persists a command before it is sent, so it survives restarts.
    pub fn push_outbox(&self, cmd: &Command) -> Result<(), CacheError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO outbox (id, body) VALUES (?1, ?2)",
            params![cmd.id.as_bytes(), postcard::to_stdvec(cmd)?],
        )?;
        Ok(())
    }

    /// Pending commands in submission order.
    pub fn outbox(&self) -> Result<Vec<Command>, CacheError> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT body FROM outbox ORDER BY n")?;
        stmt.query_map([], |r| r.get::<_, Vec<u8>>(0))?
            .map(|b| Ok(postcard::from_bytes(&b?)?))
            .collect()
    }

    /// Forgets a command once the server answered it.
    pub fn remove_outbox(&self, id: CommandId) -> Result<bool, CacheError> {
        Ok(self
            .conn
            .execute("DELETE FROM outbox WHERE id = ?1", [id.as_bytes()])?
            > 0)
    }

    /// Number of commands waiting for an answer.
    pub fn outbox_len(&self) -> Result<usize, CacheError> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM outbox", [], |r| r.get(0))?)
    }

    /// The logged-in user, remembered for offline starts.
    pub fn user(&self) -> Result<Option<User>, CacheError> {
        self.conn
            .query_row("SELECT value FROM meta WHERE key = 'user'", [], |r| {
                r.get::<_, Vec<u8>>(0)
            })
            .optional()?
            .map(|b| Ok(postcard::from_bytes(&b)?))
            .transpose()
    }

    /// Remembers the logged-in user.
    pub fn set_user(&self, user: &User) -> Result<(), CacheError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('user', ?1)",
            [postcard::to_stdvec(user)?],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use beholder_core::{
        CampaignEvent, CampaignTitle, DisplayName, Member, Role, UnixMillis, UserId,
    };
    use beholder_protocol::CommandKind;
    use uuid::Uuid;

    fn campaign() -> Campaign {
        Campaign::new(
            CampaignId(Uuid::from_bytes([1; 16])),
            CampaignTitle::new("A").unwrap(),
            Member {
                user_id: UserId(Uuid::from_bytes([2; 16])),
                display_name: DisplayName::new("dm").unwrap(),
                role: Role::Dm,
            },
        )
    }

    fn renamed(seq: u64, t: &str) -> EventEnvelope {
        EventEnvelope {
            seq: Seq(seq),
            ts: UnixMillis(0),
            actor: UserId(Uuid::from_bytes([2; 16])),
            event: CampaignEvent::Renamed {
                title: CampaignTitle::new(t).unwrap(),
            },
        }
    }

    #[test]
    fn events_apply_once_and_in_order() {
        let cache = Cache::open_in_memory().unwrap();
        let c = campaign();
        cache.put_snapshot(Seq(2), &c).unwrap();
        // seq 2 is already in the snapshot and must be skipped; gaps are fine.
        cache
            .apply_events(c.id, &[renamed(2, "old"), renamed(5, "new")])
            .unwrap();
        let (seq, got) = cache.campaigns().unwrap().remove(0);
        assert_eq!(seq, Seq(5));
        assert_eq!(got.title.as_str(), "new");
        // Replaying the same events changes nothing.
        cache.apply_events(c.id, &[renamed(5, "again")]).unwrap();
        assert_eq!(cache.campaigns().unwrap()[0].1.title.as_str(), "new");
        assert!(
            !cache
                .apply_events(CampaignId(Uuid::nil()), &[renamed(1, "x")])
                .unwrap()
        );
    }

    #[test]
    fn outbox_keeps_order_and_ignores_duplicates() {
        let cache = Cache::open_in_memory().unwrap();
        let cmds: Vec<Command> = (1..=3u8)
            .map(|n| Command {
                id: CommandId(Uuid::from_bytes([n; 16])),
                kind: CommandKind::JoinCampaign {
                    code: beholder_core::InviteCode::from_random([n; 5]),
                },
            })
            .collect();
        for c in &cmds {
            cache.push_outbox(c).unwrap();
        }
        cache.push_outbox(&cmds[0]).unwrap();
        assert_eq!(cache.outbox().unwrap(), cmds);
        assert!(cache.remove_outbox(cmds[1].id).unwrap());
        assert!(!cache.remove_outbox(cmds[1].id).unwrap());
        assert_eq!(cache.outbox_len().unwrap(), 2);
    }

    #[test]
    fn schema_mismatch_wipes_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.db");
        let cache = Cache::open(&path).unwrap();
        cache.put_snapshot(Seq(1), &campaign()).unwrap();
        cache
            .conn
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        drop(cache);
        let cache = Cache::open(&path).unwrap();
        assert!(cache.campaigns().unwrap().is_empty());
        cache.retain_campaigns(&[]).unwrap();
    }
}
