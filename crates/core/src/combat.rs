use serde::{Deserialize, Serialize};

use crate::campaign::CampaignEvent;
use crate::character::HitPoints;
use crate::ids::{CharacterId, CombatantId, EncounterId, ShapeId, UserId};
use crate::user::ValidationError;

pub const MAX_COMBATANTS: usize = 100;
pub const MAX_SHAPES: usize = 500;
pub const MAX_ENCOUNTERS: usize = 50;
pub const MAX_MAP: u16 = 200;
const MAX_TEXT: usize = 80;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    Character(CharacterId),
    Monster { source: String, key: String },
    Custom,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    pub name: String,
    pub rounds: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Health {
    Healthy,
    Bloodied,
    NearDeath,
    Down,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Combatant {
    pub id: CombatantId,
    pub name: String,
    pub source: Source,
    pub initiative: Option<i16>,
    pub hp: HitPoints,
    pub ac: Option<i16>,
    pub conditions: Vec<Condition>,
    pub concentration: bool,
    pub hidden: bool,
    pub health: Option<Health>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    Token,
    Wall,
    Zone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Colour {
    Player(u8),
    Palette(u8),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shape {
    pub id: ShapeId,
    pub kind: ShapeKind,
    pub x: i16,
    pub y: i16,
    pub w: u8,
    pub h: u8,
    pub label: String,
    pub colour: Colour,
    pub hidden: bool,
    pub combatant: Option<CombatantId>,
    pub owner: Option<UserId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Encounter {
    pub id: EncounterId,
    pub name: String,
    pub active: bool,
    pub round: u16,
    pub turn: Option<CombatantId>,
    pub combatants: Vec<Combatant>,
    pub width: u16,
    pub height: u16,
    pub shapes: Vec<Shape>,
    pub locked: bool,
    pub live_drag: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncounterOp {
    Create {
        id: EncounterId,
        name: String,
    },
    Rename {
        id: EncounterId,
        name: String,
    },
    Delete {
        id: EncounterId,
    },
    Activate {
        id: EncounterId,
        active: bool,
    },
    PutCombatant {
        id: EncounterId,
        combatant: Combatant,
    },
    RemoveCombatant {
        id: EncounterId,
        combatant: CombatantId,
    },
    SetInitiative {
        id: EncounterId,
        combatant: CombatantId,
        initiative: Option<i16>,
    },
    Damage {
        id: EncounterId,
        combatant: CombatantId,
        amount: i16,
    },
    NextTurn {
        id: EncounterId,
    },
    PreviousTurn {
        id: EncounterId,
    },
    ResetCombat {
        id: EncounterId,
    },
    Resize {
        id: EncounterId,
        width: u16,
        height: u16,
    },
    PutShape {
        id: EncounterId,
        shape: Shape,
    },
    RemoveShape {
        id: EncounterId,
        shape: ShapeId,
    },
    MoveShape {
        id: EncounterId,
        shape: ShapeId,
        x: i16,
        y: i16,
    },
    Settings {
        id: EncounterId,
        locked: bool,
        live_drag: bool,
    },
}

pub fn health(hp: HitPoints) -> Health {
    if hp.current <= 0 {
        Health::Down
    } else if i32::from(hp.current) * 4 <= i32::from(hp.max) {
        Health::NearDeath
    } else if i32::from(hp.current) * 2 <= i32::from(hp.max) {
        Health::Bloodied
    } else {
        Health::Healthy
    }
}

fn text_ok(s: &str) -> bool {
    s.chars().count() <= MAX_TEXT
}

impl Combatant {
    fn valid(&self) -> bool {
        let source_ok = match &self.source {
            Source::Monster { source, key } => text_ok(source) && text_ok(key),
            Source::Character(_) | Source::Custom => true,
        };
        !self.name.trim().is_empty()
            && text_ok(&self.name)
            && source_ok
            && self.conditions.len() <= 30
            && self.conditions.iter().all(|c| text_ok(&c.name))
            && (-10_000..=10_000).contains(&self.hp.max)
            && (-10_000..=10_000).contains(&self.hp.current)
            && (0..=10_000).contains(&self.hp.temp)
    }
}

impl Shape {
    fn valid(&self, width: u16, height: u16) -> bool {
        text_ok(&self.label)
            && self.w > 0
            && self.h > 0
            && self.x >= 0
            && self.y >= 0
            && (self.x as u16) < width
            && (self.y as u16) < height
    }
}

impl Encounter {
    pub fn new(id: EncounterId, name: String) -> Self {
        Self {
            id,
            name,
            active: false,
            round: 0,
            turn: None,
            combatants: Vec::new(),
            width: 30,
            height: 20,
            shapes: Vec::new(),
            locked: false,
            live_drag: false,
        }
    }

    pub fn order(&mut self) {
        self.combatants
            .sort_by_key(|c| (std::cmp::Reverse(c.initiative.unwrap_or(i16::MIN)), c.id));
    }

    fn turn_index(&self) -> Option<usize> {
        let t = self.turn?;
        self.combatants.iter().position(|c| c.id == t)
    }

    fn acting(&self) -> Vec<usize> {
        (0..self.combatants.len())
            .filter(|&i| self.combatants[i].initiative.is_some())
            .collect()
    }

    pub fn next_turn(&mut self) {
        let acting = self.acting();
        let Some(&first) = acting.first() else {
            return;
        };
        let current = self.turn_index();
        if let Some(i) = current {
            let ended = &mut self.combatants[i];
            ended.conditions.retain_mut(|c| match &mut c.rounds {
                Some(n) => {
                    *n = n.saturating_sub(1);
                    *n > 0
                }
                None => true,
            });
        }
        let next = current.and_then(|i| acting.iter().copied().find(|&k| k > i));
        match next {
            Some(k) => self.turn = Some(self.combatants[k].id),
            None => {
                self.round = self.round.saturating_add(1).max(1);
                self.turn = Some(self.combatants[first].id);
            }
        }
    }

    pub fn previous_turn(&mut self) {
        let acting = self.acting();
        let Some(current) = self.turn_index() else {
            return;
        };
        match acting.iter().copied().rev().find(|&k| k < current) {
            Some(k) => self.turn = Some(self.combatants[k].id),
            None if self.round > 1 => {
                self.round -= 1;
                if let Some(&last) = acting.last() {
                    self.turn = Some(self.combatants[last].id);
                }
            }
            None => {}
        }
    }

    fn valid(&self) -> bool {
        !self.name.trim().is_empty()
            && text_ok(&self.name)
            && (1..=MAX_MAP).contains(&self.width)
            && (1..=MAX_MAP).contains(&self.height)
            && self.combatants.len() <= MAX_COMBATANTS
            && self.combatants.iter().all(Combatant::valid)
            && self.shapes.len() <= MAX_SHAPES
            && self.shapes.iter().all(|s| s.valid(self.width, self.height))
    }
}

impl EncounterOp {
    pub fn id(&self) -> EncounterId {
        match self {
            Self::Create { id, .. }
            | Self::Rename { id, .. }
            | Self::Delete { id }
            | Self::Activate { id, .. }
            | Self::PutCombatant { id, .. }
            | Self::RemoveCombatant { id, .. }
            | Self::SetInitiative { id, .. }
            | Self::Damage { id, .. }
            | Self::NextTurn { id }
            | Self::PreviousTurn { id }
            | Self::ResetCombat { id }
            | Self::Resize { id, .. }
            | Self::PutShape { id, .. }
            | Self::RemoveShape { id, .. }
            | Self::MoveShape { id, .. }
            | Self::Settings { id, .. } => *id,
        }
    }

    pub fn events(&self, encounters: &[Encounter]) -> Result<Vec<CampaignEvent>, ValidationError> {
        let saved = |e: Encounter| CampaignEvent::EncounterSaved {
            encounter: Box::new(e),
        };
        let current = encounters.iter().find(|e| e.id == self.id());
        if let Self::Create { id, name } = self {
            if current.is_some() || encounters.len() >= MAX_ENCOUNTERS {
                return Err(ValidationError::Length);
            }
            let e = Encounter::new(*id, name.trim().to_owned());
            return if e.valid() {
                Ok(vec![saved(e)])
            } else {
                Err(ValidationError::Length)
            };
        }
        let Some(current) = current else {
            return Err(ValidationError::Length);
        };
        if let Self::Delete { id } = self {
            return Ok(vec![CampaignEvent::EncounterDeleted { id: *id }]);
        }
        if let Self::MoveShape { id, shape, x, y } = self {
            let s = current
                .shapes
                .iter()
                .find(|s| s.id == *shape)
                .ok_or(ValidationError::Length)?;
            let moved = Shape {
                x: *x,
                y: *y,
                ..s.clone()
            };
            if !moved.valid(current.width, current.height) {
                return Err(ValidationError::Length);
            }
            return Ok(vec![CampaignEvent::ShapeMoved {
                encounter: *id,
                shape: *shape,
                x: *x,
                y: *y,
            }]);
        }
        let mut out = Vec::new();
        let mut e = current.clone();
        match self {
            Self::Rename { name, .. } => e.name = name.trim().to_owned(),
            Self::Activate { active, .. } => {
                if *active {
                    out.extend(
                        encounters
                            .iter()
                            .filter(|o| o.active && o.id != e.id)
                            .map(|o| {
                                saved(Encounter {
                                    active: false,
                                    ..o.clone()
                                })
                            }),
                    );
                }
                e.active = *active;
            }
            Self::PutCombatant { combatant, .. } => {
                let mut c = combatant.clone();
                c.health = None;
                match e.combatants.iter_mut().find(|x| x.id == c.id) {
                    Some(x) => *x = c,
                    None => e.combatants.push(c),
                }
                e.order();
            }
            Self::RemoveCombatant { combatant, .. } => {
                if e.turn == Some(*combatant) {
                    e.next_turn();
                    if e.turn == Some(*combatant) {
                        e.turn = None;
                    }
                }
                e.combatants.retain(|c| c.id != *combatant);
                for s in &mut e.shapes {
                    if s.combatant == Some(*combatant) {
                        s.combatant = None;
                    }
                }
            }
            Self::SetInitiative {
                combatant,
                initiative,
                ..
            } => {
                let c = e
                    .combatants
                    .iter_mut()
                    .find(|c| c.id == *combatant)
                    .ok_or(ValidationError::Length)?;
                c.initiative = *initiative;
                e.order();
            }
            Self::Damage {
                combatant, amount, ..
            } => {
                let c = e
                    .combatants
                    .iter_mut()
                    .find(|c| c.id == *combatant)
                    .ok_or(ValidationError::Length)?;
                if *amount >= 0 {
                    let from_temp = (*amount).min(c.hp.temp);
                    c.hp.temp -= from_temp;
                    c.hp.current = (c.hp.current - (*amount - from_temp)).max(0);
                } else {
                    c.hp.current =
                        c.hp.current
                            .saturating_sub(*amount)
                            .min(c.hp.max.max(c.hp.current));
                }
            }
            Self::NextTurn { .. } => e.next_turn(),
            Self::PreviousTurn { .. } => e.previous_turn(),
            Self::ResetCombat { .. } => {
                e.round = 0;
                e.turn = None;
            }
            Self::Resize { width, height, .. } => {
                e.width = *width;
                e.height = *height;
                e.shapes
                    .retain(|s| (s.x as u16) < e.width && (s.y as u16) < e.height);
            }
            Self::PutShape { shape, .. } => {
                match e.shapes.iter_mut().find(|x| x.id == shape.id) {
                    Some(x) => *x = shape.clone(),
                    None => e.shapes.push(shape.clone()),
                }
                if let Some(cid) = shape.combatant
                    && let Some(c) = e.combatants.iter_mut().find(|c| c.id == cid)
                {
                    c.hidden = shape.hidden;
                }
            }
            Self::RemoveShape { shape, .. } => e.shapes.retain(|s| s.id != *shape),
            Self::Settings {
                locked, live_drag, ..
            } => {
                e.locked = *locked;
                e.live_drag = *live_drag;
            }
            Self::Create { .. } | Self::Delete { .. } | Self::MoveShape { .. } => {}
        }
        if let Self::PutCombatant { combatant, .. } = self {
            for s in &mut e.shapes {
                if s.combatant == Some(combatant.id) {
                    s.hidden = combatant.hidden;
                }
            }
        }
        if !e.valid() {
            return Err(ValidationError::Length);
        }
        out.push(saved(e));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn combatant(n: u8, initiative: Option<i16>) -> Combatant {
        Combatant {
            id: CombatantId(Uuid::from_bytes([n; 16])),
            name: format!("C{n}"),
            source: Source::Custom,
            initiative,
            hp: HitPoints {
                current: 20,
                max: 20,
                temp: 0,
            },
            ac: Some(12),
            conditions: Vec::new(),
            concentration: false,
            hidden: false,
            health: None,
        }
    }

    fn apply(list: &mut Vec<Encounter>, op: EncounterOp) {
        for ev in op.events(list).unwrap() {
            match ev {
                CampaignEvent::EncounterSaved { encounter } => {
                    match list.iter_mut().find(|e| e.id == encounter.id) {
                        Some(e) => *e = *encounter,
                        None => list.push(*encounter),
                    }
                }
                CampaignEvent::EncounterDeleted { id } => list.retain(|e| e.id != id),
                CampaignEvent::ShapeMoved { shape, x, y, .. } => {
                    for e in list.iter_mut() {
                        for s in &mut e.shapes {
                            if s.id == shape {
                                s.x = x;
                                s.y = y;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    #[test]
    fn health_bands() {
        let hp = |current, max| HitPoints {
            current,
            max,
            temp: 0,
        };
        assert_eq!(health(hp(11, 20)), Health::Healthy);
        assert_eq!(health(hp(10, 20)), Health::Bloodied);
        assert_eq!(health(hp(5, 20)), Health::NearDeath);
        assert_eq!(health(hp(0, 20)), Health::Down);
    }

    #[test]
    fn turns_rounds_and_conditions() {
        let id = EncounterId(Uuid::from_bytes([1; 16]));
        let mut list = Vec::new();
        apply(
            &mut list,
            EncounterOp::Create {
                id,
                name: "Ambush".into(),
            },
        );
        for (n, init) in [(1, Some(10)), (2, Some(18)), (3, None), (4, Some(14))] {
            apply(
                &mut list,
                EncounterOp::PutCombatant {
                    id,
                    combatant: combatant(n, init),
                },
            );
        }
        let order: Vec<String> = list[0].combatants.iter().map(|c| c.name.clone()).collect();
        assert_eq!(order, ["C2", "C4", "C1", "C3"]);
        let mut slowed = combatant(4, Some(14));
        slowed.conditions.push(Condition {
            name: "Slowed".into(),
            rounds: Some(1),
        });
        apply(
            &mut list,
            EncounterOp::PutCombatant {
                id,
                combatant: slowed,
            },
        );
        apply(&mut list, EncounterOp::NextTurn { id });
        assert_eq!(
            (list[0].round, list[0].turn),
            (1, Some(combatant(2, None).id))
        );
        apply(&mut list, EncounterOp::NextTurn { id });
        apply(&mut list, EncounterOp::NextTurn { id });
        assert_eq!(list[0].turn, Some(combatant(1, None).id));
        assert!(
            list[0].combatants[1].conditions.is_empty(),
            "the condition ran out at the end of C4's turn"
        );
        apply(&mut list, EncounterOp::NextTurn { id });
        assert_eq!(
            (list[0].round, list[0].turn),
            (2, Some(combatant(2, None).id)),
            "C3 has no initiative and is skipped"
        );
        apply(&mut list, EncounterOp::PreviousTurn { id });
        assert_eq!(
            (list[0].round, list[0].turn),
            (1, Some(combatant(1, None).id))
        );
    }

    #[test]
    fn damage_shapes_and_activation() {
        let a = EncounterId(Uuid::from_bytes([1; 16]));
        let b = EncounterId(Uuid::from_bytes([2; 16]));
        let mut list = Vec::new();
        apply(
            &mut list,
            EncounterOp::Create {
                id: a,
                name: "A".into(),
            },
        );
        apply(
            &mut list,
            EncounterOp::Create {
                id: b,
                name: "B".into(),
            },
        );
        let mut goblin = combatant(1, Some(12));
        goblin.hp.temp = 3;
        apply(
            &mut list,
            EncounterOp::PutCombatant {
                id: a,
                combatant: goblin.clone(),
            },
        );
        apply(
            &mut list,
            EncounterOp::Damage {
                id: a,
                combatant: goblin.id,
                amount: 8,
            },
        );
        assert_eq!(
            (
                list[0].combatants[0].hp.current,
                list[0].combatants[0].hp.temp
            ),
            (15, 0)
        );
        apply(
            &mut list,
            EncounterOp::Damage {
                id: a,
                combatant: goblin.id,
                amount: -30,
            },
        );
        assert_eq!(list[0].combatants[0].hp.current, 20);
        let token = Shape {
            id: ShapeId(Uuid::from_bytes([9; 16])),
            kind: ShapeKind::Token,
            x: 2,
            y: 3,
            w: 1,
            h: 1,
            label: "G".into(),
            colour: Colour::Palette(0),
            hidden: true,
            combatant: Some(goblin.id),
            owner: None,
        };
        apply(
            &mut list,
            EncounterOp::PutShape {
                id: a,
                shape: token.clone(),
            },
        );
        assert!(
            list[0].combatants[0].hidden,
            "hiding the token hides its combatant"
        );
        apply(
            &mut list,
            EncounterOp::MoveShape {
                id: a,
                shape: token.id,
                x: 5,
                y: 6,
            },
        );
        assert_eq!((list[0].shapes[0].x, list[0].shapes[0].y), (5, 6));
        assert!(
            EncounterOp::MoveShape {
                id: a,
                shape: token.id,
                x: 99,
                y: 0
            }
            .events(&list)
            .is_err(),
            "inside the map"
        );
        apply(
            &mut list,
            EncounterOp::Activate {
                id: a,
                active: true,
            },
        );
        apply(
            &mut list,
            EncounterOp::Activate {
                id: b,
                active: true,
            },
        );
        assert_eq!(list.iter().filter(|e| e.active).count(), 1);
        assert!(list.iter().find(|e| e.id == b).unwrap().active);
        apply(&mut list, EncounterOp::Delete { id: a });
        assert_eq!(list.len(), 1);
    }
}
