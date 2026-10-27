use beholder_core::compendium::{
    BUNDLE_FORMAT, Bundle, Entry, GameSystem, LIBRARY, LicenseText, RulesVersion, SourceMeta,
};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::Path;

use beholder_core::library::{LibraryEntry, LibraryEvent, LibraryOp, MAX_BATCH};
use beholder_core::{CommandId, LibraryId, UnixMillis, UserId, ValidationError};
use beholder_protocol::RejectReason;
use beholder_protocol::http::{LibraryChange, LibraryChanges};
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
    let mut last = 0;
    for event in &events {
        last = emit(tx, actor, cmd_id, event, now)?;
    }
    Ok(Ok(last))
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
