use std::collections::HashMap;
use std::path::Path;

use beholder_core::compendium::{
    Alternative, BUNDLE_FORMAT, Bundle, Entry, FacetOptions, Facets, Hit, Kind, LIBRARY,
    LibraryMeta, LicenseText, RulesVersion, SearchQuery, SourceMeta,
};
use beholder_core::library::LibraryEvent;
use beholder_core::{Campaign, CampaignId, CommandId, Seq, User};
use beholder_protocol::http::LibraryChange;
use beholder_protocol::{Command, EventEnvelope};
use rusqlite::{Connection, OptionalExtension, params};

const SCHEMA_VERSION: i64 = 5;

const SCHEMA: &str = "
CREATE TABLE meta (key TEXT PRIMARY KEY, value BLOB NOT NULL) WITHOUT ROWID;
-- Confirmed state only: server events and snapshots, never optimistic changes.
CREATE TABLE campaigns (id BLOB PRIMARY KEY, last_seq INTEGER NOT NULL, state BLOB NOT NULL) WITHOUT ROWID;
-- Pending commands in submission order; deleted on Ack or Reject.
CREATE TABLE outbox (n INTEGER PRIMARY KEY AUTOINCREMENT, id BLOB NOT NULL UNIQUE, body BLOB NOT NULL);
";

const COMPENDIUM_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS compendium_sources (
    id TEXT PRIMARY KEY, sha256 TEXT NOT NULL, seq INTEGER NOT NULL, meta BLOB NOT NULL
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS compendium_licenses (
    id TEXT PRIMARY KEY, license BLOB NOT NULL
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS compendium_entries (
    rowid INTEGER PRIMARY KEY, source TEXT NOT NULL, key TEXT NOT NULL, kind INTEGER NOT NULL,    name TEXT NOT NULL, facets BLOB NOT NULL, text TEXT NOT NULL, extra BLOB NOT NULL,
    UNIQUE (source, key)
);
CREATE VIRTUAL TABLE IF NOT EXISTS compendium_fts USING fts5(
    name, text, content = 'compendium_entries', content_rowid = 'rowid',
    tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3'
);
";

const MAX_CANDIDATES: usize = 2000;

type Extra = (Option<String>, Option<RulesVersion>, Option<LibraryMeta>);

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("corrupt cache entry")]
    Corrupt,
}

impl From<postcard::Error> for CacheError {
    fn from(_: postcard::Error) -> Self {
        Self::Corrupt
    }
}

pub struct Cache {
    conn: Connection,
}

impl Cache {
    pub fn open(path: &Path) -> Result<Self, CacheError> {
        Self::init(Connection::open(path)?)
    }

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
        let format: Option<i64> = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'compendium_format'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if format != Some(i64::from(BUNDLE_FORMAT)) {
            conn.execute_batch(
                "DROP TABLE IF EXISTS compendium_fts; DROP TABLE IF EXISTS compendium_entries;
                 DROP TABLE IF EXISTS compendium_sources; DROP TABLE IF EXISTS compendium_licenses;",
            )?;
            conn.execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES ('compendium_format', ?1)",
                [i64::from(BUNDLE_FORMAT)],
            )?;
        }
        conn.execute_batch(COMPENDIUM_SCHEMA)?;
        Ok(Self { conn })
    }

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

    pub fn put_snapshot(&self, seq: Seq, campaign: &Campaign) -> Result<(), CacheError> {
        self.put(seq, campaign)
    }

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

    pub fn remove_campaign(&self, id: CampaignId) -> Result<(), CacheError> {
        self.conn
            .execute("DELETE FROM campaigns WHERE id = ?1", [id.as_bytes()])?;
        Ok(())
    }

    pub fn retain_campaigns(&self, keep: &[CampaignId]) -> Result<(), CacheError> {
        for (_, c) in self.campaigns()? {
            if !keep.contains(&c.id) {
                self.remove_campaign(c.id)?;
            }
        }
        Ok(())
    }

    pub fn push_outbox(&self, cmd: &Command) -> Result<(), CacheError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO outbox (id, body) VALUES (?1, ?2)",
            params![cmd.id.as_bytes(), postcard::to_stdvec(cmd)?],
        )?;
        Ok(())
    }

    pub fn outbox(&self) -> Result<Vec<Command>, CacheError> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT n, body FROM outbox ORDER BY n")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        let mut out = Vec::with_capacity(rows.len());
        for (n, body) in rows {
            match postcard::from_bytes(&body) {
                Ok(cmd) => out.push(cmd),
                // A command queued by an older app version can't be sent any more; drop it so it can't block the queue.
                Err(_) => {
                    self.conn.execute("DELETE FROM outbox WHERE n = ?1", [n])?;
                }
            }
        }
        Ok(out)
    }

    pub fn remove_outbox(&self, id: CommandId) -> Result<bool, CacheError> {
        Ok(self
            .conn
            .execute("DELETE FROM outbox WHERE id = ?1", [id.as_bytes()])?
            > 0)
    }

    pub fn outbox_len(&self) -> Result<usize, CacheError> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM outbox", [], |r| r.get(0))?)
    }

    pub fn user(&self) -> Result<Option<User>, CacheError> {
        self.conn
            .query_row("SELECT value FROM meta WHERE key = 'user'", [], |r| {
                r.get::<_, Vec<u8>>(0)
            })
            .optional()?
            .map(|b| Ok(postcard::from_bytes(&b)?))
            .transpose()
    }

    pub fn set_user(&self, user: &User) -> Result<(), CacheError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('user', ?1)",
            [postcard::to_stdvec(user)?],
        )?;
        Ok(())
    }
}

impl Cache {
    pub fn compendium_sha256(&self, source: &str) -> Result<Option<String>, CacheError> {
        Ok(self
            .conn
            .query_row(
                "SELECT sha256 FROM compendium_sources WHERE id = ?1",
                [source],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn put_bundle(&self, sha256: &str, bundle: &Bundle) -> Result<(), CacheError> {
        let tx = self.conn.unchecked_transaction()?;
        let id = bundle.source.id.as_str();
        tx.execute("DELETE FROM compendium_entries WHERE source = ?1", [id])?;
        {
            let mut insert = tx.prepare_cached(
                "INSERT INTO compendium_entries (source, key, kind, name, facets, text, extra)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for e in &bundle.entries {
                insert.execute(params![
                    id,
                    e.key,
                    kind_to_int(e.facets.kind()),
                    e.name,
                    postcard::to_stdvec(&e.facets)?,
                    e.text,
                    extra(e)?
                ])?;
            }
        }
        tx.execute(            "INSERT OR REPLACE INTO compendium_sources (id, sha256, seq, meta) VALUES (?1, ?2, ?3, ?4)",
            params![id, sha256, bundle.seq, postcard::to_stdvec(&bundle.source)?],
        )?;
        for l in &bundle.licenses {
            tx.execute(
                "INSERT OR REPLACE INTO compendium_licenses (id, license) VALUES (?1, ?2)",
                params![l.id, postcard::to_stdvec(l)?],
            )?;
        }
        tx.execute_batch("INSERT INTO compendium_fts (compendium_fts) VALUES ('rebuild')")?;
        tx.commit()?;
        Ok(())
    }

    pub fn retain_sources(&self, keep: &[String]) -> Result<bool, CacheError> {
        let gone: Vec<String> = self
            .compendium_sources()?
            .into_iter()
            .map(|m| m.id)
            .filter(|id| !keep.contains(id))
            .collect();
        if gone.is_empty() {
            return Ok(false);
        }
        let tx = self.conn.unchecked_transaction()?;
        for id in &gone {
            tx.execute("DELETE FROM compendium_entries WHERE source = ?1", [id])?;
            tx.execute("DELETE FROM compendium_sources WHERE id = ?1", [id])?;
        }
        tx.execute_batch("INSERT INTO compendium_fts (compendium_fts) VALUES ('rebuild')")?;
        tx.commit()?;
        Ok(true)
    }

    pub fn compendium_sources(&self) -> Result<Vec<SourceMeta>, CacheError> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT meta FROM compendium_sources ORDER BY id")?;
        stmt.query_map([], |r| r.get::<_, Vec<u8>>(0))?
            .map(|row| Ok(postcard::from_bytes(&row?)?))
            .collect()
    }

    pub fn search(&self, q: &SearchQuery, limit: usize) -> Result<Vec<Hit>, CacheError> {
        let mut hits = Vec::new();
        let mut push = |row: &rusqlite::Row<'_>| -> Result<(), CacheError> {
            let source: String = row.get(0)?;
            let facets: Facets = postcard::from_bytes(&row.get::<_, Vec<u8>>(3)?)?;
            let key: String = row.get(1)?;
            let hidden = source == LIBRARY && q.hidden.contains(&key);
            if !hidden && q.matches(&source, &facets) {
                hits.push(Hit {
                    source,
                    key,
                    name: row.get(2)?,
                    facets,
                    others: 0,
                });
            }
            Ok(())
        };
        match fts_query(&q.text) {
            Some(m) => {
                let mut stmt = self.conn.prepare_cached(
                    "SELECT e.source, e.key, e.name, e.facets FROM compendium_fts f
                     JOIN compendium_entries e ON e.rowid = f.rowid
                     WHERE compendium_fts MATCH ?1
                     ORDER BY bm25(compendium_fts, 10.0, 1.0) LIMIT ?2",
                )?;
                let mut rows = stmt.query(params![m, MAX_CANDIDATES])?;
                while let Some(row) = rows.next()? {
                    push(row)?;
                }
            }
            None => {
                let mut stmt = self.conn.prepare_cached(
                    "SELECT source, key, name, facets FROM compendium_entries
                     WHERE ?1 IS NULL OR kind = ?1 ORDER BY name COLLATE NOCASE",
                )?;
                let mut rows = stmt.query([q.kind.map(kind_to_int)])?;
                while let Some(row) = rows.next()? {
                    push(row)?;
                }
            }
        }
        let needle = q.text.trim().to_lowercase();
        if !needle.is_empty() {
            hits.sort_by_key(|h| !h.name.to_lowercase().starts_with(&needle));
        }
        let mut hits = fold_duplicates(hits, &self.priorities()?);
        hits.truncate(limit);
        Ok(hits)
    }

    pub fn compendium_entry(
        &self,
        source: &str,
        key: &str,
    ) -> Result<Option<(Entry, SourceMeta)>, CacheError> {
        let row = self
            .conn
            .query_row(
                "SELECT e.name, e.facets, e.text, s.meta, e.extra FROM compendium_entries e
                 JOIN compendium_sources s ON s.id = e.source
                 WHERE e.source = ?1 AND e.key = ?2",
                [source, key],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Vec<u8>>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, Vec<u8>>(3)?,
                        r.get::<_, Vec<u8>>(4)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(name, facets, text, meta, extra)| {
            let (book, rules, library): Extra = postcard::from_bytes(&extra)?;
            Ok((
                Entry {
                    key: key.to_owned(),
                    name,
                    facets: postcard::from_bytes(&facets)?,
                    text,
                    book,
                    rules,
                    library,
                },
                postcard::from_bytes(&meta)?,
            ))
        })
        .transpose()
    }

    pub fn facet_options(&self) -> Result<FacetOptions, CacheError> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT facets FROM compendium_entries WHERE kind IN (?1, ?2, ?3)")?;
        let mut opts = FacetOptions::default();
        let rows = stmt.query_map(
            params![
                kind_to_int(Kind::Spell),
                kind_to_int(Kind::MagicItem),
                kind_to_int(Kind::Class)
            ],
            |r| r.get::<_, Vec<u8>>(0),
        )?;
        let keep = |list: &mut Vec<String>, v: &str| {
            if !list.iter().any(|x| x == v) {
                list.push(v.to_owned());
            }
        };
        for row in rows {
            match postcard::from_bytes::<Facets>(&row?)? {
                Facets::Spell {
                    school, classes, ..
                } => {
                    keep(&mut opts.schools, &school);
                    for c in &classes {
                        keep(&mut opts.classes, c);
                    }
                }
                Facets::MagicItem { rarity, .. } => keep(&mut opts.rarities, &rarity),
                _ => {}
            }
        }
        opts.schools.sort();
        opts.classes.sort();
        opts.rarities.sort();
        Ok(opts)
    }
}

impl Cache {
    fn priorities(&self) -> Result<HashMap<String, u8>, CacheError> {
        Ok(self
            .compendium_sources()?
            .into_iter()
            .map(|m| {
                let p = m.priority();
                (m.id, p)
            })
            .collect())
    }

    pub fn alternatives(
        &self,
        source: &str,
        key: &str,
        allowed: &[String],
    ) -> Result<Vec<Alternative>, CacheError> {
        let names: HashMap<String, (String, u8)> = self
            .compendium_sources()?
            .into_iter()
            .map(|m| {
                let p = m.priority();
                (m.id, (m.name, p))
            })
            .collect();
        let mut stmt = self.conn.prepare_cached(
            "SELECT o.source, o.key, o.facets, o.extra FROM compendium_entries e
             JOIN compendium_entries o ON o.kind = e.kind AND o.name = e.name COLLATE NOCASE
             WHERE e.source = ?1 AND e.key = ?2 AND NOT (o.source = ?1 AND o.key = ?2)",
        )?;
        let rows = stmt.query_map([source, key], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Vec<u8>>(2)?,
                r.get::<_, Vec<u8>>(3)?,
            ))
        })?;
        let mut out: Vec<(u8, Alternative)> = Vec::new();
        for row in rows {
            let (src, k, facets, extra) = row?;
            if !allowed.contains(&src) {
                continue;
            }
            let (book, _, _): Extra = postcard::from_bytes(&extra)?;
            let facets = postcard::from_bytes(&facets)?;
            let (name, p) = names.get(&src).cloned().unwrap_or_default();
            out.push((
                p,
                Alternative {
                    source: src,
                    source_name: name,
                    key: k,
                    book,
                    facets,
                },
            ));
        }
        out.sort_by(|a, b| (a.0, &a.1.source_name).cmp(&(b.0, &b.1.source_name)));
        Ok(out.into_iter().map(|(_, a)| a).collect())
    }

    pub fn compendium_licenses(&self) -> Result<Vec<LicenseText>, CacheError> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT license FROM compendium_licenses ORDER BY id")?;
        stmt.query_map([], |r| r.get::<_, Vec<u8>>(0))?
            .map(|row| Ok(postcard::from_bytes(&row?)?))
            .collect()
    }
}

fn extra(e: &Entry) -> Result<Vec<u8>, CacheError> {
    let x: Extra = (e.book.clone(), e.rules, e.library);
    Ok(postcard::to_stdvec(&x)?)
}

impl Cache {
    pub fn compendium_seq(&self, source: &str) -> Result<Option<u64>, CacheError> {
        Ok(self
            .conn
            .query_row(
                "SELECT seq FROM compendium_sources WHERE id = ?1",
                [source],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn apply_library(&self, changes: &[LibraryChange], head: u64) -> Result<(), CacheError> {
        let tx = self.conn.unchecked_transaction()?;
        for c in changes {
            let id = match &c.event {
                LibraryEvent::Saved { entry } => entry.id,
                LibraryEvent::Deleted { id } => *id,
            };
            let key = id.to_string();
            let old: Option<(i64, String, String)> = tx
                .query_row(
                    "SELECT rowid, name, text FROM compendium_entries WHERE source = ?1 AND key = ?2",
                    [LIBRARY, key.as_str()],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            if let Some((rowid, name, text)) = &old {
                tx.execute(
                    "INSERT INTO compendium_fts (compendium_fts, rowid, name, text) VALUES ('delete', ?1, ?2, ?3)",
                    params![rowid, name, text],
                )?;
                tx.execute("DELETE FROM compendium_entries WHERE rowid = ?1", [rowid])?;
            }
            if let LibraryEvent::Saved { entry } = &c.event {
                let e = entry.to_entry();
                tx.execute(
                    "INSERT INTO compendium_entries (source, key, kind, name, facets, text, extra)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        LIBRARY,
                        e.key,
                        kind_to_int(e.facets.kind()),
                        e.name,
                        postcard::to_stdvec(&e.facets)?,
                        e.text,
                        extra(&e)?
                    ],
                )?;
                tx.execute(
                    "INSERT INTO compendium_fts (rowid, name, text) VALUES (last_insert_rowid(), ?1, ?2)",
                    params![e.name, e.text],
                )?;
            }
        }
        tx.execute(
            "UPDATE compendium_sources SET seq = ?2 WHERE id = ?1",
            params![LIBRARY, head],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn library_entries(&self) -> Result<Vec<Entry>, CacheError> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT key, name, facets, text, extra FROM compendium_entries WHERE source = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map([LIBRARY], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Vec<u8>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Vec<u8>>(4)?,
            ))
        })?;
        rows.map(|row| {
            let (key, name, facets, text, extra) = row?;
            let (book, rules, library): Extra = postcard::from_bytes(&extra)?;
            Ok(Entry {
                key,
                name,
                facets: postcard::from_bytes(&facets)?,
                text,
                book,
                rules,
                library,
            })
        })
        .collect()
    }
}

fn fold_duplicates(hits: Vec<Hit>, priority: &HashMap<String, u8>) -> Vec<Hit> {
    let rank = |h: &Hit| priority.get(&h.source).copied().unwrap_or(u8::MAX);
    let mut index: HashMap<(Kind, String), usize> = HashMap::new();
    let mut out: Vec<Hit> = Vec::with_capacity(hits.len());
    for h in hits {
        let group = (h.facets.kind(), h.name.to_lowercase());
        match index.get(&group) {
            Some(&i) => {
                let best = &mut out[i];
                best.others += 1;
                if rank(&h) < rank(best) {
                    let others = best.others;
                    *best = Hit { others, ..h };
                }
            }
            None => {
                index.insert(group, out.len());
                out.push(h);
            }
        }
    }
    out
}

fn kind_to_int(k: Kind) -> i64 {
    Kind::ALL
        .iter()
        .position(|x| *x == k)
        .map_or(-1, |i| i as i64)
}

fn fts_query(text: &str) -> Option<String> {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let terms: Vec<String> = cleaned
        .split_whitespace()
        .map(|t| format!("\"{t}\"*"))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
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
        cache
            .apply_events(c.id, &[renamed(2, "old"), renamed(5, "new")])
            .unwrap();
        let (seq, got) = cache.campaigns().unwrap().remove(0);
        assert_eq!(seq, Seq(5));
        assert_eq!(got.title.as_str(), "new");
        cache.apply_events(c.id, &[renamed(5, "again")]).unwrap();
        assert_eq!(cache.campaigns().unwrap()[0].1.title.as_str(), "new");
        assert!(
            !cache
                .apply_events(CampaignId(Uuid::nil()), &[renamed(1, "x")])
                .unwrap()
        );
    }

    #[test]
    fn unreadable_outbox_commands_are_dropped() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .conn
            .execute(
                "INSERT INTO outbox (id, body) VALUES (?1, ?2)",
                rusqlite::params![[9u8; 16].as_slice(), [255u8, 255, 255].as_slice()],
            )
            .unwrap();
        assert!(cache.outbox().unwrap().is_empty());
        assert_eq!(cache.outbox_len().unwrap(), 0, "the bad row is gone");
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

    fn srd() -> Bundle {
        let bytes = include_bytes!("../../../data/srd-5.2/srd.bundle");
        let raw = zstd::bulk::decompress(bytes, 64 << 20).unwrap();
        postcard::from_bytes(&raw).unwrap()
    }

    #[test]
    fn compendium_search() {
        let cache = Cache::open_in_memory().unwrap();
        assert_eq!(cache.compendium_sha256("srd-5.2").unwrap(), None);
        cache.put_bundle("aa", &srd()).unwrap();
        cache.put_bundle("bb", &srd()).unwrap();
        assert_eq!(
            cache.compendium_sha256("srd-5.2").unwrap().as_deref(),
            Some("bb")
        );

        let q = |text: &str| SearchQuery {
            text: text.into(),
            sources: vec!["srd-5.2".into()],
            ..SearchQuery::default()
        };
        let hits = cache.search(&q("fireb"), 20).unwrap();
        assert_eq!(
            hits.first().map(|h| h.key.as_str()),
            Some("spell/fireball"),
            "{hits:?}"
        );
        let names: Vec<String> = cache
            .search(&q("magic missile"), 5)
            .unwrap()
            .into_iter()
            .map(|h| h.name)
            .collect();
        assert_eq!(names.first().map(String::as_str), Some("Magic Missile"));
        assert!(
            cache.search(&q("\"; DROP TABLE x --"), 5).is_ok(),
            "FTS syntax can't be injected"
        );

        let wizard_3 = SearchQuery {
            sources: vec!["srd-5.2".into()],
            kind: Some(Kind::Spell),
            level: Some(3),
            class: Some("Wizard".into()),
            ..SearchQuery::default()
        };
        let hits = cache.search(&wizard_3, 500).unwrap();
        assert!(hits.iter().any(|h| h.key == "spell/fireball"));
        assert!(hits.iter().all(|h| matches!(&h.facets, Facets::Spell { level: 3, classes, .. } if classes.iter().any(|c| c == "Wizard"))));
        let all_monsters = SearchQuery {
            sources: vec!["srd-5.2".into()],
            kind: Some(Kind::Monster),
            ..SearchQuery::default()
        };
        assert_eq!(cache.search(&all_monsters, 10_000).unwrap().len(), 341);

        let (entry, meta) = cache
            .compendium_entry("srd-5.2", "spell/fireball")
            .unwrap()
            .unwrap();
        assert_eq!(entry.name, "Fireball");
        assert!(meta.attribution.contains("Creative Commons"));
        let opts = cache.facet_options().unwrap();
        assert!(opts.schools.contains(&"Evocation".to_owned()));
        assert!(opts.classes.contains(&"Wizard".to_owned()));
        assert!(opts.rarities.contains(&"Legendary".to_owned()));

        assert!(!cache.retain_sources(&["srd-5.2".into()]).unwrap());
        assert!(cache.retain_sources(&[]).unwrap());
        assert!(cache.search(&q("fireball"), 5).unwrap().is_empty());
        assert!(cache.compendium_sources().unwrap().is_empty());
    }

    #[test]
    fn duplicates_fold_by_priority_and_list_alternatives() {
        use beholder_core::compendium::RulesVersion;
        let cache = Cache::open_in_memory().unwrap();
        let srd = srd();
        let mut old = srd.clone();
        old.source.id = "old-srd".into();
        old.source.name = "Old SRD".into();
        old.source.rules = RulesVersion::R2014;
        old.entries
            .retain(|e| e.key == "spell/fireball" || e.key == "spell/magic-missile");
        cache.put_bundle("aa", &old).unwrap();
        cache.put_bundle("bb", &srd).unwrap();
        let both = vec!["old-srd".to_owned(), "srd-5.2".to_owned()];
        let q = SearchQuery {
            text: "fireball".into(),
            sources: both.clone(),
            ..SearchQuery::default()
        };
        let hits = cache.search(&q, 10).unwrap();
        let fb: Vec<&Hit> = hits.iter().filter(|h| h.name == "Fireball").collect();
        assert_eq!(fb.len(), 1, "one row per name");
        assert_eq!(
            fb[0].source, "srd-5.2",
            "the 2024 SRD wins over 2014 content"
        );
        assert_eq!(fb[0].others, 1);
        let only_old = SearchQuery {
            sources: vec!["old-srd".into()],
            ..q.clone()
        };
        let hits = cache.search(&only_old, 10).unwrap();
        assert_eq!(hits[0].source, "old-srd", "disabled sources never show");
        assert_eq!(hits[0].others, 0);
        let alts = cache
            .alternatives("srd-5.2", "spell/fireball", &both)
            .unwrap();
        assert_eq!(alts.len(), 1);
        assert_eq!(alts[0].source_name, "Old SRD");
        assert!(
            cache
                .alternatives("srd-5.2", "spell/fireball", &["srd-5.2".into()])
                .unwrap()
                .is_empty()
        );
        assert_eq!(cache.compendium_licenses().unwrap().len(), 1);
    }
}
