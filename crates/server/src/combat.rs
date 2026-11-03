use std::collections::HashMap;

use beholder_core::combat::{Colour, Combatant, Encounter, EncounterOp, Shape, ShapeKind, Source};
use beholder_core::{CampaignEvent, CampaignId, EncounterId, Role};
use beholder_core::{CombatantId, ShapeId, UserId};
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
            EncounterOp::PutShape { shape, .. } => permissions::can_put_shape(viewer, e, shape),
            EncounterOp::RemoveShape { shape, .. } => {
                permissions::can_remove_shape(viewer, e, *shape)
            }
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

#[derive(serde::Deserialize)]
struct LegacyShape {
    id: ShapeId,
    kind: ShapeKind,
    x: i16,
    y: i16,
    w: u8,
    h: u8,
    label: String,
    colour: Colour,
    hidden: bool,
    combatant: Option<CombatantId>,
    owner: Option<UserId>,
}

#[derive(serde::Deserialize)]
struct Legacy {
    id: EncounterId,
    name: String,
    active: bool,
    round: u16,
    turn: Option<CombatantId>,
    combatants: Vec<Combatant>,
    width: u16,
    height: u16,
    shapes: Vec<LegacyShape>,
    locked: bool,
    live_drag: bool,
}

impl From<Legacy> for Encounter {
    fn from(l: Legacy) -> Self {
        Self {
            id: l.id,
            name: l.name,
            active: l.active,
            round: l.round,
            turn: l.turn,
            combatants: l.combatants,
            width: l.width,
            height: l.height,
            shapes: l
                .shapes
                .into_iter()
                .map(|s| Shape {
                    id: s.id,
                    kind: s.kind,
                    x: s.x,
                    y: s.y,
                    w: s.w,
                    h: s.h,
                    label: s.label,
                    colour: s.colour,
                    hidden: s.hidden,
                    combatant: s.combatant,
                    owner: s.owner,
                    note: String::new(),
                })
                .collect(),
            locked: l.locked,
            live_drag: l.live_drag,
        }
    }
}

fn decode(body: &[u8]) -> Result<Encounter, DbError> {
    match postcard::from_bytes::<Encounter>(body) {
        Ok(e) => Ok(e),
        Err(_) => Ok(postcard::from_bytes::<Legacy>(body)?.into()),
    }
}

fn load(conn: &Connection, id: EncounterId) -> Result<Option<Encounter>, DbError> {
    conn.query_row(
        "SELECT body FROM encounters WHERE id = ?1",
        [id.as_bytes()],
        |r| r.get::<_, Vec<u8>>(0),
    )
    .optional()?
    .map(|b| decode(&b))
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
        .map(|row| decode(&row?))
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

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn encounters_saved_before_notes_still_load() {
        #[derive(serde::Serialize)]
        struct OldShape {
            id: ShapeId,
            kind: ShapeKind,
            x: i16,
            y: i16,
            w: u8,
            h: u8,
            label: String,
            colour: Colour,
            hidden: bool,
            combatant: Option<CombatantId>,
            owner: Option<UserId>,
        }
        #[derive(serde::Serialize)]
        struct Old {
            id: EncounterId,
            name: String,
            active: bool,
            round: u16,
            turn: Option<CombatantId>,
            combatants: Vec<Combatant>,
            width: u16,
            height: u16,
            shapes: Vec<OldShape>,
            locked: bool,
            live_drag: bool,
        }
        let old = Old {
            id: EncounterId(Uuid::from_bytes([1; 16])),
            name: "Old".into(),
            active: true,
            round: 0,
            turn: None,
            combatants: Vec::new(),
            width: 30,
            height: 20,
            shapes: vec![OldShape {
                id: ShapeId(Uuid::from_bytes([2; 16])),
                kind: ShapeKind::Wall,
                x: 1,
                y: 2,
                w: 3,
                h: 1,
                label: "Wall".into(),
                colour: Colour::Palette(0),
                hidden: false,
                combatant: None,
                owner: None,
            }],
            locked: false,
            live_drag: true,
        };
        let e = decode(&postcard::to_stdvec(&old).unwrap()).unwrap();
        assert_eq!(
            (e.name.as_str(), e.shapes.len(), e.live_drag),
            ("Old", 1, true)
        );
        assert_eq!(
            (e.shapes[0].label.as_str(), e.shapes[0].note.as_str()),
            ("Wall", "")
        );
    }
}
