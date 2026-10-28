use beholder_core::compendium::{
    BUNDLE_FORMAT, Bundle, Entry, GameSystem, LIBRARY, LicenseText, RulesVersion, SourceMeta,
};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::Path;

use beholder_core::library::{LibraryDraft, LibraryEntry, LibraryEvent, LibraryOp, MAX_BATCH};
use beholder_core::{CommandId, ImportId, LibraryId, UnixMillis, UserId, ValidationError};
use beholder_protocol::RejectReason;
use beholder_protocol::http::{ImportInfo, LibraryChange, LibraryChanges};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::db::DbError;
use crate::permissions;

pub const MAX_REPLAY: u64 = 1000;

pub(crate) fn decide(
    tx: &Transaction<'_>,
    actor: UserId,
    cmd_id: CommandId,
    op: &LibraryOp,
    now: UnixMillis,
) -> Result<Result<u64, RejectReason>, DbError> {
    let events = match op {
        LibraryOp::Save {
            draft,
            base_version,
        } => {
            if let Err(e) = draft.check_facets() {
                return Ok(Err(RejectReason::Invalid(e)));
            }
            let entry = match (load(tx, draft.id)?, base_version) {
                (None, 0) => LibraryEntry {
                    id: draft.id,
                    author: actor,
                    name: draft.name.clone(),
                    facets: draft.facets.clone(),
                    text: draft.text.clone(),
                    book: draft.book.clone(),
                    rules: draft.rules,
                    version: 1,
                },
                (None, _) | (Some((_, true)), _) => return Ok(Err(RejectReason::NotFound)),
                (Some(_), 0) => return Ok(Err(RejectReason::IdConflict)),
                (Some((current, false)), base) => {
                    if !permissions::can_edit_library(
                        actor,
                        current.author,
                        is_dm_anywhere(tx, actor)?,
                    ) {
                        return Ok(Err(RejectReason::Forbidden));
                    }
                    if current.version != *base {
                        return Ok(Err(RejectReason::Stale));
                    }
                    let Some(version) = current.version.checked_add(1) else {
                        return Ok(Err(RejectReason::Invalid(ValidationError::Length)));
                    };
                    LibraryEntry {
                        id: draft.id,
                        author: current.author,
                        name: draft.name.clone(),
                        facets: draft.facets.clone(),
                        text: draft.text.clone(),
                        book: draft.book.clone(),
                        rules: draft.rules,
                        version,
                    }
                }
            };
            vec![LibraryEvent::Saved { entry }]
        }
        LibraryOp::SaveMany { drafts } => {
            if drafts.is_empty() || drafts.len() > MAX_BATCH {
                return Ok(Err(RejectReason::Invalid(ValidationError::Length)));
            }
            let mut ids = HashSet::with_capacity(drafts.len());
            for d in drafts {
                if let Err(e) = d.check_facets() {
                    return Ok(Err(RejectReason::Invalid(e)));
                }
                if !ids.insert(d.id) || load(tx, d.id)?.is_some() {
                    return Ok(Err(RejectReason::IdConflict));
                }
            }
            drafts
                .iter()
                .map(|d| LibraryEvent::Saved {
                    entry: LibraryEntry {
                        id: d.id,
                        author: actor,
                        name: d.name.clone(),
                        facets: d.facets.clone(),
                        text: d.text.clone(),
                        book: d.book.clone(),
                        rules: d.rules,
                        version: 1,
                    },
                })
                .collect()
        }
        LibraryOp::UpsertMany { items } => match upsert_events(tx, actor, items)? {
            Ok(events) => events,
            Err(reason) => return Ok(Err(reason)),
        },
        LibraryOp::ImportMany { import, items } => {
            let events = match upsert_events(tx, actor, items)? {
                Ok(events) => events,
                Err(reason) => return Ok(Err(reason)),
            };
            if let Err(reason) = start_import(tx, *import, actor, now)? {
                return Ok(Err(reason));
            }
            let mut last = 0;
            for event in &events {
                let LibraryEvent::Saved { entry } = event else {
                    continue;
                };
                let prev = entry_seq(tx, entry.id)?;
                last = emit(tx, actor, cmd_id, event, now)?;
                tx.execute(
                    "INSERT INTO library_import_items (import_id, entry_id, prev_seq, seq) VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT (import_id, entry_id) DO UPDATE SET seq = excluded.seq",
                    params![import.as_bytes(), entry.id.as_bytes(), prev, last],
                )?;
            }
            tx.execute(
                "UPDATE library_imports SET entries = (SELECT COUNT(*) FROM library_import_items WHERE import_id = ?1) WHERE id = ?1",
                [import.as_bytes()],
            )?;
            return Ok(Ok(last));
        }
        LibraryOp::UndoImport { import } => match undo_events(tx, actor, *import)? {
            Ok(events) => events,
            Err(reason) => return Ok(Err(reason)),
        },
        LibraryOp::Delete { id } => {
            let Some((current, false)) = load(tx, *id)? else {
                return Ok(Err(RejectReason::NotFound));
            };
            if !permissions::can_edit_library(actor, current.author, is_dm_anywhere(tx, actor)?) {
                return Ok(Err(RejectReason::Forbidden));
            }
            vec![LibraryEvent::Deleted { id: *id }]
        }
    };
    let mut last = head(tx)?;
    for event in &events {
        last = emit(tx, actor, cmd_id, event, now)?;
    }
    Ok(Ok(last))
}

fn upsert_events(
    tx: &Transaction<'_>,
    actor: UserId,
    items: &[(LibraryDraft, u32)],
) -> Result<Result<Vec<LibraryEvent>, RejectReason>, DbError> {
    if items.is_empty() || items.len() > MAX_BATCH {
        return Ok(Err(RejectReason::Invalid(ValidationError::Length)));
    }
    let dm = is_dm_anywhere(tx, actor)?;
    let mut ids = HashSet::with_capacity(items.len());
    let mut events = Vec::with_capacity(items.len());
    for (d, base) in items {
        if let Err(e) = d.check_facets() {
            return Ok(Err(RejectReason::Invalid(e)));
        }
        if !ids.insert(d.id) {
            return Ok(Err(RejectReason::IdConflict));
        }
        let (author, version) = match (load(tx, d.id)?, *base) {
            (None, 0) => (actor, 1),
            (None, _) | (Some((_, true)), _) => return Ok(Err(RejectReason::NotFound)),
            (Some(_), 0) => return Ok(Err(RejectReason::IdConflict)),
            (Some((current, false)), base) => {
                if !permissions::can_edit_library(actor, current.author, dm) {
                    return Ok(Err(RejectReason::Forbidden));
                }
                if current.version != base {
                    return Ok(Err(RejectReason::Stale));
                }
                let Some(next) = base.checked_add(1) else {
                    return Ok(Err(RejectReason::Invalid(ValidationError::Length)));
                };
                (current.author, next)
            }
        };
        events.push(LibraryEvent::Saved {
            entry: LibraryEntry {
                id: d.id,
                author,
                name: d.name.clone(),
                facets: d.facets.clone(),
                text: d.text.clone(),
                book: d.book.clone(),
                rules: d.rules,
                version,
            },
        });
    }
    Ok(Ok(events))
}

fn start_import(
    tx: &Transaction<'_>,
    import: ImportId,
    actor: UserId,
    now: UnixMillis,
) -> Result<Result<(), RejectReason>, DbError> {
    tx.execute(
        "INSERT INTO library_imports (id, actor, ts) VALUES (?1, ?2, ?3) ON CONFLICT (id) DO NOTHING",
        params![import.as_bytes(), actor.as_bytes(), now.0],
    )?;
    let (owner, undone): (Vec<u8>, bool) = tx.query_row(
        "SELECT actor, undone FROM library_imports WHERE id = ?1",
        [import.as_bytes()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(if owner != actor.as_bytes() {
        Err(RejectReason::Forbidden)
    } else if undone {
        Err(RejectReason::Stale)
    } else {
        Ok(())
    })
}

fn entry_seq(conn: &Connection, id: LibraryId) -> Result<Option<u64>, DbError> {
    Ok(conn
        .query_row(
            "SELECT seq FROM library_entries WHERE id = ?1",
            [id.as_bytes()],
            |r| r.get(0),
        )
        .optional()?)
}

fn undo_events(
    tx: &Transaction<'_>,
    actor: UserId,
    import: ImportId,
) -> Result<Result<Vec<LibraryEvent>, RejectReason>, DbError> {
    let Some((importer, undone)) = tx
        .query_row(
            "SELECT actor, undone FROM library_imports WHERE id = ?1",
            [import.as_bytes()],
            |r| Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, bool>(1)?)),
        )
        .optional()?
    else {
        return Ok(Err(RejectReason::NotFound));
    };
    let importer = UserId(uuid::Uuid::from_slice(&importer).map_err(|_| DbError::Corrupt("uuid"))?);
    if !permissions::can_undo_import(actor, importer, is_dm_anywhere(tx, actor)?) {
        return Ok(Err(RejectReason::Forbidden));
    }
    if undone {
        return Ok(Err(RejectReason::Stale));
    }
    let mut stmt = tx.prepare(
        "SELECT i.entry_id, i.prev_seq, i.seq, e.seq FROM library_import_items i
         JOIN library_entries e ON e.id = i.entry_id WHERE i.import_id = ?1",
    )?;
    let rows = stmt
        .query_map([import.as_bytes()], |r| {
            Ok((
                r.get::<_, Vec<u8>>(0)?,
                r.get::<_, Option<u64>>(1)?,
                r.get::<_, u64>(2)?,
                r.get::<_, u64>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    let mut events = Vec::with_capacity(rows.len());
    let mut kept = 0u32;
    for (id, prev, written, current) in rows {
        if written != current {
            kept += 1;
            continue;
        }
        let id = LibraryId(uuid::Uuid::from_slice(&id).map_err(|_| DbError::Corrupt("uuid"))?);
        let Some(prev) = prev else {
            events.push(LibraryEvent::Deleted { id });
            continue;
        };
        let body: Vec<u8> =
            tx.query_row("SELECT body FROM library_log WHERE seq = ?1", [prev], |r| {
                r.get(0)
            })?;
        let (LibraryEvent::Saved { entry: mut old }, Some((now_entry, false))) =
            (postcard::from_bytes(&body)?, load(tx, id)?)
        else {
            kept += 1;
            continue;
        };
        let Some(version) = now_entry.version.checked_add(1) else {
            kept += 1;
            continue;
        };
        old.version = version;
        events.push(LibraryEvent::Saved { entry: old });
    }
    tx.execute(
        "UPDATE library_imports SET undone = 1, kept = ?2 WHERE id = ?1",
        params![import.as_bytes(), kept],
    )?;
    Ok(Ok(events))
}

pub fn imports(conn: &Connection) -> Result<Vec<ImportInfo>, DbError> {
    let mut stmt = conn.prepare_cached(
        "SELECT i.id, i.actor, COALESCE(u.display_name, ''), i.ts, i.entries, i.undone, i.kept
         FROM library_imports i LEFT JOIN users u ON u.id = i.actor
         ORDER BY i.ts DESC LIMIT 50",
    )?;
    stmt.query_map([], |r| {
        Ok((
            r.get::<_, Vec<u8>>(0)?,
            r.get::<_, Vec<u8>>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, u32>(4)?,
            r.get::<_, bool>(5)?,
            r.get::<_, u32>(6)?,
        ))
    })?
    .map(|row| {
        let (id, actor, actor_name, ts, entries, undone, kept) = row?;
        let uuid = |b: &[u8]| uuid::Uuid::from_slice(b).map_err(|_| DbError::Corrupt("uuid"));
        Ok(ImportInfo {
            id: ImportId(uuid(&id)?),
            actor: UserId(uuid(&actor)?),
            actor_name,
            ts: UnixMillis(ts),
            entries,
            undone,
            kept,
        })
    })
    .collect()
}

fn emit(
    tx: &Transaction<'_>,
    actor: UserId,
    cmd_id: CommandId,
    event: &LibraryEvent,
    now: UnixMillis,
) -> Result<u64, DbError> {
    let seq: u64 = tx.query_row(
        "INSERT INTO library_log (ts, actor, command_id, body) VALUES (?1, ?2, ?3, ?4) RETURNING seq",
        params![
            now.0,
            actor.as_bytes(),
            cmd_id.as_bytes(),
            postcard::to_stdvec(event)?
        ],
        |r| r.get(0),
    )?;
    match event {
        LibraryEvent::Saved { entry } => {
            tx.execute(
                "INSERT INTO library_entries (id, body, deleted, seq) VALUES (?1, ?2, 0, ?3)
                 ON CONFLICT (id) DO UPDATE SET body = excluded.body, seq = excluded.seq",
                params![entry.id.as_bytes(), postcard::to_stdvec(entry)?, seq],
            )?;
        }
        LibraryEvent::Deleted { id } => {
            tx.execute(
                "UPDATE library_entries SET deleted = 1, seq = ?2 WHERE id = ?1",
                params![id.as_bytes(), seq],
            )?;
        }
    }
    Ok(seq)
}

fn load(conn: &Connection, id: LibraryId) -> Result<Option<(LibraryEntry, bool)>, DbError> {
    conn.query_row(
        "SELECT body, deleted FROM library_entries WHERE id = ?1",
        [id.as_bytes()],
        |r| Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, bool>(1)?)),
    )
    .optional()?
    .map(|(body, deleted)| Ok((postcard::from_bytes(&body)?, deleted)))
    .transpose()
}

fn is_dm_anywhere(conn: &Connection, user: UserId) -> Result<bool, DbError> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM memberships WHERE user_id = ?1 AND role = 0 LIMIT 1",
            [user.as_bytes()],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

pub fn head(conn: &Connection) -> Result<u64, DbError> {
    Ok(
        conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM library_log", [], |r| {
            r.get(0)
        })?,
    )
}

pub fn changes(conn: &Connection, after: u64) -> Result<LibraryChanges, DbError> {
    let head = head(conn)?;
    if after > head || head - after > MAX_REPLAY {
        return Ok(LibraryChanges {
            head,
            changes: None,
        });
    }
    let mut stmt =
        conn.prepare_cached("SELECT seq, body FROM library_log WHERE seq > ?1 ORDER BY seq")?;
    let changes = stmt
        .query_map([after], |r| {
            Ok((r.get::<_, u64>(0)?, r.get::<_, Vec<u8>>(1)?))
        })?
        .map(|row| {
            let (seq, body) = row?;
            Ok(LibraryChange {
                seq,
                event: postcard::from_bytes(&body)?,
            })
        })
        .collect::<Result<_, DbError>>()?;
    Ok(LibraryChanges {
        head,
        changes: Some(changes),
    })
}

pub fn source_meta() -> SourceMeta {
    SourceMeta {
        id: LIBRARY.into(),
        name: "Server library".into(),
        publisher: "This server".into(),
        system: GameSystem::Dnd5e,
        rules: RulesVersion::R2024,
        licenses: Vec::new(),
        upstream: String::new(),
        attribution: "Entries added by this server's users. They stay on this server and are never bundled with the app.".into(),
    }
}

pub fn bundle(conn: &Connection) -> Result<Bundle, DbError> {
    let seq = head(conn)?;
    let mut stmt =
        conn.prepare_cached("SELECT body FROM library_entries WHERE deleted = 0 ORDER BY id")?;
    let entries = stmt
        .query_map([], |r| r.get::<_, Vec<u8>>(0))?
        .map(|row| Ok(postcard::from_bytes::<LibraryEntry>(&row?)?.to_entry()))
        .collect::<Result<Vec<Entry>, DbError>>()?;
    Ok(Bundle {
        format: BUNDLE_FORMAT,
        seq,
        source: source_meta(),
        licenses: Vec::new(),
        entries,
    })
}

struct Hashing<W> {
    inner: W,
    sha: Sha256,
    bytes: u64,
}

impl<W: std::io::Write> std::io::Write for Hashing<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.sha.update(&buf[..n]);
        self.bytes += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

pub struct Written {
    pub seq: u64,
    pub bytes: u64,
    pub sha256: String,
}

// Byte-identical to postcard(Bundle): header fields, then varint(count), then each entry.
pub fn write_bundle(conn: &Connection, path: &Path) -> Result<Written, DbError> {
    use std::io::Write;
    let seq = head(conn)?;
    let count: u64 = conn.query_row(
        "SELECT COUNT(*) FROM library_entries WHERE deleted = 0",
        [],
        |r| r.get(0),
    )?;
    let tmp = path.with_extension("tmp");
    let file = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
    let mut z = zstd::stream::write::Encoder::new(
        Hashing {
            inner: file,
            sha: Sha256::new(),
            bytes: 0,
        },
        3,
    )?;
    z.write_all(&postcard::to_stdvec(&(
        BUNDLE_FORMAT,
        seq,
        source_meta(),
        Vec::<LicenseText>::new(),
    ))?)?;
    z.write_all(&postcard::to_stdvec(&(count as usize))?)?;
    let mut stmt =
        conn.prepare_cached("SELECT body FROM library_entries WHERE deleted = 0 ORDER BY id")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let entry: LibraryEntry = postcard::from_bytes(&row.get::<_, Vec<u8>>(0)?)?;
        z.write_all(&postcard::to_stdvec(&entry.to_entry())?)?;
    }
    let mut out = z.finish()?;
    out.flush()?;
    let Hashing { inner, sha, bytes } = out;
    drop(inner);
    let digest = sha.finalize();
    std::fs::rename(&tmp, path)?;
    Ok(Written {
        seq,
        bytes,
        sha256: digest.iter().map(|b| format!("{b:02x}")).collect(),
    })
}
