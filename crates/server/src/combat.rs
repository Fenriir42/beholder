use std::collections::HashMap;

use beholder_core::character::HitPoints;
use beholder_core::combat::{
    Colour, Combatant, Condition, Encounter, EncounterOp, Health, Shape, ShapeKind, Source, TurnUse,
};
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
            EncounterOp::SetInitiative { combatant, .. }
            | EncounterOp::UseTurn { combatant, .. } => {
                let characters = crate::characters::campaign_characters(tx, campaign_id)?;
                permissions::acts_for(viewer, e, *combatant, &characters)
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

impl From<LegacyShape> for Shape {
    fn from(s: LegacyShape) -> Self {
        Self {
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
        }
    }
}

#[derive(serde::Deserialize)]
struct LegacyCombatant {
    id: CombatantId,
    name: String,
    source: Source,
    initiative: Option<i16>,
    hp: HitPoints,
    ac: Option<i16>,
    conditions: Vec<Condition>,
    concentration: bool,
    hidden: bool,
    health: Option<Health>,
}

impl From<LegacyCombatant> for Combatant {
    fn from(c: LegacyCombatant) -> Self {
        Self {
            id: c.id,
            name: c.name,
            source: c.source,
            initiative: c.initiative,
            hp: c.hp,
            ac: c.ac,
            conditions: c.conditions,
            concentration: c.concentration,
            hidden: c.hidden,
            health: c.health,
            speed: None,
            used: TurnUse::default(),
        }
    }
}

#[derive(serde::Deserialize)]
struct Legacy<S> {
    id: EncounterId,
    name: String,
    active: bool,
    round: u16,
    turn: Option<CombatantId>,
    combatants: Vec<LegacyCombatant>,
    width: u16,
    height: u16,
    shapes: Vec<S>,
    locked: bool,
    live_drag: bool,
}

impl<S: Into<Shape>> From<Legacy<S>> for Encounter {
    fn from(l: Legacy<S>) -> Self {
        Self {
            id: l.id,
            name: l.name,
            active: l.active,
            round: l.round,
            turn: l.turn,
            combatants: l.combatants.into_iter().map(Into::into).collect(),
            width: l.width,
            height: l.height,
            shapes: l.shapes.into_iter().map(Into::into).collect(),
            locked: l.locked,
            live_drag: l.live_drag,
        }
    }
}

// Encounters stored by older servers: protocol 8 (no shape notes) and 9 (no turn use or speed).
fn decode(body: &[u8]) -> Result<Encounter, DbError> {
    if let Ok(e) = postcard::from_bytes::<Encounter>(body) {
        return Ok(e);
    }
    if let Ok(e) = postcard::from_bytes::<Legacy<Shape>>(body) {
        return Ok(e.into());
    }
    Ok(postcard::from_bytes::<Legacy<LegacyShape>>(body)?.into())
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
                e.move_shape(*shape, *x, *y);
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

pub enum AsPlayer {
    Forbidden,
    NotFound,
    View(Option<Encounter>),
}

pub fn encounter_as(
    conn: &mut Connection,
    campaign_id: CampaignId,
    requester: beholder_core::UserId,
    id: EncounterId,
    player: beholder_core::UserId,
) -> Result<AsPlayer, DbError> {
    let tx = conn.transaction()?;
    let Some(role) = crate::campaigns::role_of(&tx, campaign_id, requester)? else {
        return Ok(AsPlayer::NotFound);
    };
    if !permissions::can_preview_as(Viewer {
        user: requester,
        role,
    }) {
        return Ok(AsPlayer::Forbidden);
    }
    let Some(Role::Player) = crate::campaigns::role_of(&tx, campaign_id, player)? else {
        return Ok(AsPlayer::NotFound);
    };
    let Some(settings) = crate::campaigns::settings_of(&tx, campaign_id)? else {
        return Ok(AsPlayer::NotFound);
    };
    let found = campaign_encounters(&tx, campaign_id)?
        .into_iter()
        .find(|e| e.id == id);
    let Some(e) = found else {
        return Ok(AsPlayer::NotFound);
    };
    let viewer = Viewer {
        user: player,
        role: Role::Player,
    };
    Ok(AsPlayer::View(permissions::encounter_for(
        viewer, &settings, &e,
    )))
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
    fn encounters_saved_before_turn_use_still_load() {
        #[derive(serde::Serialize)]
        struct OldCombatant {
            id: CombatantId,
            name: String,
            source: Source,
            initiative: Option<i16>,
            hp: HitPoints,
            ac: Option<i16>,
            conditions: Vec<Condition>,
            concentration: bool,
            hidden: bool,
            health: Option<Health>,
        }
        #[derive(serde::Serialize)]
        struct Old {
            id: EncounterId,
            name: String,
            active: bool,
            round: u16,
            turn: Option<CombatantId>,
            combatants: Vec<OldCombatant>,
            width: u16,
            height: u16,
            shapes: Vec<Shape>,
            locked: bool,
            live_drag: bool,
        }
        let old = Old {
            id: EncounterId(Uuid::from_bytes([1; 16])),
            name: "Nine".into(),
            active: false,
            round: 2,
            turn: None,
            combatants: vec![OldCombatant {
                id: CombatantId(Uuid::from_bytes([3; 16])),
                name: "Goblin".into(),
                source: Source::Custom,
                initiative: Some(12),
                hp: HitPoints {
                    current: 7,
                    max: 7,
                    temp: 0,
                },
                ac: Some(15),
                conditions: Vec::new(),
                concentration: false,
                hidden: false,
                health: None,
            }],
            width: 30,
            height: 20,
            shapes: Vec::new(),
            locked: false,
            live_drag: false,
        };
        let e = decode(&postcard::to_stdvec(&old).unwrap()).unwrap();
        assert_eq!((e.round, e.combatants[0].name.as_str()), (2, "Goblin"));
        assert_eq!(
            (e.combatants[0].speed, e.combatants[0].used),
            (None, TurnUse::default())
        );
    }

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
