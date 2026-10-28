use beholder_core::character::{Character, CharacterOp};
use beholder_core::{CampaignEvent, CampaignId, CharacterId};
use beholder_protocol::RejectReason;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::db::DbError;
use crate::permissions::{self, Viewer};

pub(crate) fn decide(
    tx: &Transaction<'_>,
    viewer: Viewer,
    campaign_id: CampaignId,
    op: &CharacterOp,
) -> Result<Result<Vec<CampaignEvent>, RejectReason>, DbError> {
    let characters = campaign_characters(tx, campaign_id)?;
    match op {
        CharacterOp::Create { id, .. } => {
            if exists(tx, *id)? {
                return Ok(Err(RejectReason::IdConflict));
            }
        }
        CharacterOp::Change { id, .. }
        | CharacterOp::SetActive { id }
        | CharacterOp::Delete { id } => {
            let Some(c) = characters.iter().find(|c| c.id == *id) else {
                return Ok(Err(RejectReason::NotFound));
            };
            if !permissions::can_edit_character(viewer, c) {
                return Ok(Err(RejectReason::Forbidden));
            }
        }
    }
    Ok(op
        .events(viewer.user, &characters)
        .map_err(RejectReason::Invalid))
}

fn exists(conn: &Connection, id: CharacterId) -> Result<bool, DbError> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM characters WHERE id = ?1",
            [id.as_bytes()],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

pub(crate) fn project(
    tx: &Transaction<'_>,
    campaign_id: CampaignId,
    event: &CampaignEvent,
) -> Result<(), DbError> {
    match event {
        CampaignEvent::CharacterSaved { character } => {
            tx.execute(
                "INSERT INTO characters (id, campaign_id, owner, body) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (id) DO UPDATE SET body = excluded.body",
                params![
                    character.id.as_bytes(),
                    campaign_id.as_bytes(),
                    character.owner.as_bytes(),
                    postcard::to_stdvec(&**character)?
                ],
            )?;
        }
        CampaignEvent::CharacterDeleted { id } => {
            tx.execute(
                "UPDATE characters SET deleted = 1 WHERE id = ?1",
                [id.as_bytes()],
            )?;
        }
        _ => {}
    }
    Ok(())
}

pub fn campaign_characters(
    conn: &Connection,
    campaign_id: CampaignId,
) -> Result<Vec<Character>, DbError> {
    let mut stmt = conn.prepare_cached(
        "SELECT body FROM characters WHERE campaign_id = ?1 AND deleted = 0 ORDER BY id",
    )?;
    stmt.query_map([campaign_id.as_bytes()], |r| r.get::<_, Vec<u8>>(0))?
        .map(|row| Ok(postcard::from_bytes(&row?)?))
        .collect()
}
