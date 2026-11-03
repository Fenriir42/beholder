use std::collections::HashMap;

use beholder_core::combat::{Encounter, EncounterOp, Source};
use beholder_core::{CampaignEvent, CampaignId, EncounterId, Role};
use beholder_protocol::RejectReason;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::db::DbError;
use crate::permissions::{self, Viewer};

pub(crate) fn decide(
    tx: &Transaction<'_>,
    viewer: Viewer,
    campaign_id: CampaignId,
    op: &EncounterOp,
) -> Result<Result<Vec<CampaignEvent>, RejectReason>, DbError> {
    let encounters = campaign_encounters(tx, campaign_id)?;
    if viewer.role != Role::Dm {
        let Some(e) = encounters.iter().find(|e| e.id == op.id() && e.active) else {
            return Ok(Err(RejectReason::NotFound));
        };
        let allowed = match op {
            EncounterOp::MoveShape { shape, .. } => permissions::can_move_shape(viewer, e, *shape),
            EncounterOp::SetInitiative { combatant, .. } => {
                let characters = crate::characters::campaign_characters(tx, campaign_id)?;
                e.combatants
                    .iter()
                    .find(|c| c.id == *combatant)
                    .is_some_and(|c| match c.source {
                        Source::Character(id) => characters
                            .iter()
                            .any(|ch| ch.id == id && ch.owner == viewer.user),
                        _ => false,
                    })
            }
            _ => false,
        };
        if !allowed {
            return Ok(Err(RejectReason::Forbidden));
        }
    }
    Ok(op.events(&encounters).map_err(RejectReason::Invalid))
}

fn load(conn: &Connection, id: EncounterId) -> Result<Option<Encounter>, DbError> {
    conn.query_row(
        "SELECT body FROM encounters WHERE id = ?1",
        [id.as_bytes()],
        |r| r.get::<_, Vec<u8>>(0),
    )
    .optional()?
    .map(|b| Ok(postcard::from_bytes(&b)?))
    .transpose()
}

fn save(tx: &Transaction<'_>, campaign_id: CampaignId, e: &Encounter) -> Result<(), DbError> {
    tx.execute(
        "INSERT INTO encounters (id, campaign_id, body) VALUES (?1, ?2, ?3)
         ON CONFLICT (id) DO UPDATE SET body = excluded.body",
        params![
            e.id.as_bytes(),
            campaign_id.as_bytes(),
            postcard::to_stdvec(e)?
        ],
    )?;
    Ok(())
}

pub(crate) fn project(
    tx: &Transaction<'_>,
    campaign_id: CampaignId,
    event: &CampaignEvent,
) -> Result<(), DbError> {
    match event {
        CampaignEvent::EncounterSaved { encounter } => save(tx, campaign_id, encounter)?,
        CampaignEvent::EncounterDeleted { id } => {
            tx.execute("DELETE FROM encounters WHERE id = ?1", [id.as_bytes()])?;
        }
        CampaignEvent::ShapeMoved {
            encounter,
            shape,
            x,
            y,
        } => {
            if let Some(mut e) = load(tx, *encounter)? {
                if let Some(s) = e.shapes.iter_mut().find(|s| s.id == *shape) {
                    s.x = *x;
                    s.y = *y;
                }
                save(tx, campaign_id, &e)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn campaign_encounters(
    conn: &Connection,
    campaign_id: CampaignId,
) -> Result<Vec<Encounter>, DbError> {
    let mut stmt =
        conn.prepare_cached("SELECT body FROM encounters WHERE campaign_id = ?1 ORDER BY id")?;
    stmt.query_map([campaign_id.as_bytes()], |r| r.get::<_, Vec<u8>>(0))?
        .map(|row| Ok(postcard::from_bytes(&row?)?))
        .collect()
}

pub fn encounter(conn: &Connection, id: EncounterId) -> Result<Option<Encounter>, DbError> {
    load(conn, id)
}

pub fn touched_encounters<'a>(
    conn: &Connection,
    events: impl IntoIterator<Item = &'a CampaignEvent>,
) -> Result<HashMap<EncounterId, Encounter>, DbError> {
    let mut out = HashMap::new();
    for e in events {
        if let CampaignEvent::ShapeMoved { encounter, .. } = e
            && !out.contains_key(encounter)
            && let Some(found) = load(conn, *encounter)?
        {
            out.insert(*encounter, found);
        }
    }
    Ok(out)
}
