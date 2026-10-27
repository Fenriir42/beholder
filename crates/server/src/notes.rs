use std::collections::HashMap;

use beholder_core::{
    Audience, CampaignEvent, CampaignId, CampaignSettings, Note, NoteBody, NoteId, NoteOp,
    NoteTitle, UserId, ValidationError,
};
use beholder_protocol::RejectReason;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::campaigns::{blob_id, role_of};
use crate::db::DbError;
use crate::permissions::{self, Viewer};

pub(crate) fn decide(
    tx: &Transaction<'_>,
    viewer: Viewer,
    campaign_id: CampaignId,
    settings: &CampaignSettings,
    op: &NoteOp,
    conflict_id: NoteId,
) -> Result<Result<Option<CampaignEvent>, RejectReason>, DbError> {
    let stored = load_note(tx, op.note_id())?;
    if let NoteOp::Create { audience, .. } = op {
        if stored.is_some() {
            return Ok(Err(RejectReason::IdConflict));
        }
        if let Err(r) = check_audience(tx, campaign_id, viewer.user, audience)? {
            return Ok(Err(r));
        }
        return Ok(op
            .predicted_event(viewer.user, None)
            .map(Some)
            .ok_or(RejectReason::Invalid(ValidationError::Length)));
    }
    let Some(note) = stored
        .filter(|s| s.campaign_id == campaign_id && !s.deleted)
        .map(|s| s.note)
        .filter(|n| permissions::can_read_note(viewer, settings, n))
    else {
        return Ok(Err(RejectReason::NotFound));
    };
    match op {
        NoteOp::Create { .. } => Ok(Err(RejectReason::IdConflict)),
        NoteOp::Edit {
            base_version,
            title,
            body,
            ..
        } => {
            if !permissions::can_edit_note(viewer, &note) {
                return Ok(Err(RejectReason::Forbidden));
            }
            if note.version != *base_version {
                return Ok(Ok(Some(CampaignEvent::NoteCreated {
                    note: Note {
                        id: conflict_id,
                        author: viewer.user,
                        title: title.clone(),
                        body: body.clone(),
                        audience: Audience::Private,
                        version: 1,
                        conflict_of: Some(note.id),
                    },
                })));
            }
            Ok(op
                .predicted_event(viewer.user, Some(&note))
                .map(Some)
                .ok_or(RejectReason::Invalid(ValidationError::Length)))
        }
        NoteOp::SetAudience { audience, .. } => {
            if !permissions::can_manage_note(viewer, &note) {
                return Ok(Err(RejectReason::Forbidden));
            }
            if let Err(r) = check_audience(tx, campaign_id, note.author, audience)? {
                return Ok(Err(r));
            }
            Ok(Ok(op.predicted_event(viewer.user, Some(&note))))
        }
        NoteOp::Delete { .. } => {
            if !permissions::can_manage_note(viewer, &note) {
                return Ok(Err(RejectReason::Forbidden));
            }
            Ok(Ok(op.predicted_event(viewer.user, Some(&note))))
        }
    }
}

fn check_audience(
    tx: &Transaction<'_>,
    campaign_id: CampaignId,
    author: UserId,
    audience: &Audience,
) -> Result<Result<(), RejectReason>, DbError> {
    let audience = match audience.clone().normalized(author) {
        Ok(a) => a,
        Err(e) => return Ok(Err(RejectReason::Invalid(e))),
    };
    if let Audience::Users(users) = &audience {
        for u in users {
            if role_of(tx, campaign_id, *u)?.is_none() {
                return Ok(Err(RejectReason::NotFound));
            }
        }
    }
    Ok(Ok(()))
}

pub(crate) fn project(
    tx: &Transaction<'_>,
    campaign_id: CampaignId,
    event: &CampaignEvent,
) -> Result<(), DbError> {
    match event {
        CampaignEvent::NoteCreated { note } => {
            tx.execute(
                "INSERT INTO notes (id, campaign_id, author, title, body, audience, version, conflict_of)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    note.id.as_bytes(),
                    campaign_id.as_bytes(),
                    note.author.as_bytes(),
                    note.title.as_str(),
                    note.body.as_str(),
                    postcard::to_stdvec(&note.audience)?,
                    note.version,
                    note.conflict_of.as_ref().map(|c| c.as_bytes().as_slice()),
                ],
            )?;
        }
        CampaignEvent::NoteEdited {
            id,
            title,
            body,
            version,
        } => {
            tx.execute(
                "UPDATE notes SET title = ?2, body = ?3, version = ?4 WHERE id = ?1",
                params![id.as_bytes(), title.as_str(), body.as_str(), version],
            )?;
        }
        CampaignEvent::NoteAudienceChanged { id, audience, .. } => {
            tx.execute(
                "UPDATE notes SET audience = ?2 WHERE id = ?1",
                params![id.as_bytes(), postcard::to_stdvec(audience)?],
            )?;
        }
        CampaignEvent::NoteDeleted { id } => {
            tx.execute(
                "UPDATE notes SET deleted = 1, body = '' WHERE id = ?1",
                [id.as_bytes()],
            )?;
        }
        CampaignEvent::Created { .. }
        | CampaignEvent::Renamed { .. }
        | CampaignEvent::CoverChanged { .. }
        | CampaignEvent::MemberJoined { .. }
        | CampaignEvent::InviteRegenerated { .. }
        | CampaignEvent::SettingsChanged { .. }
        | CampaignEvent::SourcesChanged { .. }
        | CampaignEvent::EntryHidden { .. } => {}
    }
    Ok(())
}

struct Stored {
    campaign_id: CampaignId,
    deleted: bool,
    note: Note,
}

const COLUMNS: &str =
    "id, author, title, body, audience, version, conflict_of, campaign_id, deleted";

fn raw_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<RawRow> {
    Ok(RawRow {
        id: r.get(0)?,
        author: r.get(1)?,
        title: r.get(2)?,
        body: r.get(3)?,
        audience: r.get(4)?,
        version: r.get(5)?,
        conflict_of: r.get(6)?,
        campaign_id: r.get(7)?,
        deleted: r.get(8)?,
    })
}

struct RawRow {
    id: Vec<u8>,
    author: Vec<u8>,
    title: String,
    body: String,
    audience: Vec<u8>,
    version: u32,
    conflict_of: Option<Vec<u8>>,
    campaign_id: Vec<u8>,
    deleted: bool,
}

impl RawRow {
    fn decode(self) -> Result<Stored, DbError> {
        Ok(Stored {
            campaign_id: CampaignId(blob_id(&self.campaign_id)?),
            deleted: self.deleted,
            note: Note {
                id: NoteId(blob_id(&self.id)?),
                author: UserId(blob_id(&self.author)?),
                title: NoteTitle::new(&self.title).map_err(|_| DbError::Corrupt("note title"))?,
                body: NoteBody::new(self.body).map_err(|_| DbError::Corrupt("note body"))?,
                audience: postcard::from_bytes(&self.audience)?,
                version: self.version,
                conflict_of: self
                    .conflict_of
                    .map(|b| blob_id(&b).map(NoteId))
                    .transpose()?,
            },
        })
    }
}

fn load_note(conn: &Connection, id: NoteId) -> Result<Option<Stored>, DbError> {
    conn.prepare_cached(&format!("SELECT {COLUMNS} FROM notes WHERE id = ?1"))?
        .query_row([id.as_bytes()], raw_row)
        .optional()?
        .map(RawRow::decode)
        .transpose()
}

pub fn campaign_notes(conn: &Connection, campaign_id: CampaignId) -> Result<Vec<Note>, DbError> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM notes WHERE campaign_id = ?1 AND deleted = 0 ORDER BY id"
    ))?;
    stmt.query_map([campaign_id.as_bytes()], raw_row)?
        .map(|row| Ok(row?.decode()?.note))
        .collect()
}

pub fn touched_notes<'a>(
    conn: &Connection,
    events: impl IntoIterator<Item = &'a CampaignEvent>,
) -> Result<HashMap<NoteId, Note>, DbError> {
    let mut out = HashMap::new();
    for e in events {
        let id = match e {
            CampaignEvent::NoteEdited { id, .. }
            | CampaignEvent::NoteAudienceChanged { id, .. }
            | CampaignEvent::NoteDeleted { id } => *id,
            _ => continue,
        };
        if out.contains_key(&id) {
            continue;
        }
        if let Some(s) = load_note(conn, id)? {
            out.insert(id, s.note);
        }
    }
    Ok(out)
}
