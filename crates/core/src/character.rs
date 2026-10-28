use serde::{Deserialize, Serialize};

use crate::campaign::CampaignEvent;
use crate::compendium::RulesVersion;
use crate::ids::{CharacterId, UserId};
use crate::note::MAX_BODY_BYTES;
use crate::rules::{
    self, Ability, ArmorFormula, Caster, Proficiency, Skill, armor_class, modifier,
    proficiency_bonus, proficiency_part,
};
use crate::user::ValidationError;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub name: String,
    pub source: Option<String>,
    pub key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassLevel {
    pub class: Link,
    pub subclass: Option<Link>,
    pub level: u8,
    pub hit_die: u8,
    pub caster: Caster,
    pub spell_ability: Option<Ability>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HitPoints {
    pub current: i16,
    pub max: i16,
    pub temp: i16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counter {
    pub name: String,
    pub current: u16,
    pub max: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownSpell {
    pub spell: Link,
    pub level: u8,
    pub prepared: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub item: Link,
    pub quantity: u32,
    pub equipped: bool,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Character {
    pub id: CharacterId,
    pub owner: UserId,
    pub name: String,
    pub rules: RulesVersion,
    pub species: Option<Link>,
    pub background: Option<Link>,
    pub classes: Vec<ClassLevel>,
    pub feats: Vec<Link>,
    pub features: Vec<Link>,
    pub abilities: [u8; 6],
    pub saves: [bool; 6],
    pub skills: [Proficiency; 18],
    pub jack_of_all_trades: bool,
    pub armor: ArmorFormula,
    pub shield: bool,
    pub ac_bonus: i8,
    pub initiative_bonus: i8,
    pub speed: u16,
    pub hp: HitPoints,
    pub hit_dice_spent: u8,
    pub death_saves: (u8, u8),
    pub conditions: Vec<String>,
    pub exhaustion: u8,
    pub counters: Vec<Counter>,
    pub slots_spent: [u8; 9],
    pub pact_spent: u8,
    pub spells: Vec<KnownSpell>,
    pub inventory: Vec<Item>,
    pub currency: [u32; 5],
    pub notes: String,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterSummary {
    pub id: CharacterId,
    pub owner: UserId,
    pub name: String,
    pub species: Option<String>,
    pub classes: Vec<(String, u8)>,
    pub armor_class: i16,
    pub hp: HitPoints,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CharacterChange {
    Name(String),
    Rules(RulesVersion),
    Species(Option<Link>),
    Background(Option<Link>),
    Classes(Vec<ClassLevel>),
    Feats(Vec<Link>),
    Features(Vec<Link>),
    Abilities([u8; 6]),
    Saves([bool; 6]),
    Skill(Skill, Proficiency),
    JackOfAllTrades(bool),
    Armor {
        formula: ArmorFormula,
        shield: bool,
        bonus: i8,
    },
    InitiativeBonus(i8),
    Speed(u16),
    Hp(HitPoints),
    HitDiceSpent(u8),
    DeathSaves(u8, u8),
    Conditions(Vec<String>),
    Exhaustion(u8),
    Counters(Vec<Counter>),
    SlotsSpent([u8; 9]),
    PactSpent(u8),
    Spells(Vec<KnownSpell>),
    Inventory(Vec<Item>),
    Currency([u32; 5]),
    Notes(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CharacterOp {
    Create {
        id: CharacterId,
        name: String,
        rules: RulesVersion,
    },
    Change {
        id: CharacterId,
        change: CharacterChange,
    },
    SetActive {
        id: CharacterId,
    },
    Delete {
        id: CharacterId,
    },
}

const MAX_NAME: usize = 80;
const MAX_TEXT: usize = 120;
const MAX_LIST: usize = 200;
const MAX_INVENTORY: usize = 500;

fn text_ok(s: &str) -> bool {
    s.chars().count() <= MAX_TEXT
}

fn link_ok(l: &Link) -> bool {
    !l.name.trim().is_empty()
        && text_ok(&l.name)
        && l.source.as_deref().is_none_or(text_ok)
        && l.key.as_deref().is_none_or(text_ok)
}

impl CharacterOp {
    pub fn id(&self) -> CharacterId {
        match self {
            Self::Create { id, .. }
            | Self::Change { id, .. }
            | Self::SetActive { id }
            | Self::Delete { id } => *id,
        }
    }

    pub fn events(
        &self,
        actor: UserId,
        characters: &[Character],
    ) -> Result<Vec<CampaignEvent>, ValidationError> {
        let saved = |c: Character| CampaignEvent::CharacterSaved {
            character: Box::new(c),
        };
        let current = characters.iter().find(|c| c.id == self.id());
        match (self, current) {
            (Self::Create { id, name, rules }, None) => {
                let mut c = Character::new(*id, actor, name.trim().to_owned(), *rules);
                c.active = !characters.iter().any(|o| o.owner == actor && o.active);
                c.validate()?;
                Ok(vec![saved(c)])
            }
            (Self::Change { change, .. }, Some(c)) => {
                let mut next = c.clone();
                next.apply(change.clone());
                next.validate()?;
                Ok(vec![saved(next)])
            }
            (Self::SetActive { .. }, Some(c)) => {
                let mut out: Vec<CampaignEvent> = characters
                    .iter()
                    .filter(|o| o.owner == c.owner && o.active && o.id != c.id)
                    .map(|o| {
                        saved(Character {
                            active: false,
                            ..o.clone()
                        })
                    })
                    .collect();
                if !c.active {
                    out.push(saved(Character {
                        active: true,
                        ..c.clone()
                    }));
                }
                Ok(out)
            }
            (Self::Delete { id }, Some(_)) => Ok(vec![CampaignEvent::CharacterDeleted { id: *id }]),
            _ => Err(ValidationError::Length),
        }
    }
}

impl Character {
    pub fn new(id: CharacterId, owner: UserId, name: String, rules: RulesVersion) -> Self {
        Self {
            id,
            owner,
            name,
            rules,
            species: None,
            background: None,
            classes: Vec::new(),
            feats: Vec::new(),
            features: Vec::new(),
            abilities: [10; 6],
            saves: [false; 6],
            skills: [Proficiency::None; 18],
            jack_of_all_trades: false,
            armor: ArmorFormula::Unarmored,
            shield: false,
            ac_bonus: 0,
            initiative_bonus: 0,
            speed: 30,
            hp: HitPoints::default(),
            hit_dice_spent: 0,
            death_saves: (0, 0),
            conditions: Vec::new(),
            exhaustion: 0,
            counters: Vec::new(),
            slots_spent: [0; 9],
            pact_spent: 0,
            spells: Vec::new(),
            inventory: Vec::new(),
            currency: [0; 5],
            notes: String::new(),
            active: false,
        }
    }

    pub fn apply(&mut self, change: CharacterChange) {
        match change {
            CharacterChange::Name(n) => self.name = n.trim().to_owned(),
            CharacterChange::Rules(r) => self.rules = r,
            CharacterChange::Species(l) => self.species = l,
            CharacterChange::Background(l) => self.background = l,
            CharacterChange::Classes(c) => self.classes = c,
            CharacterChange::Feats(f) => self.feats = f,
            CharacterChange::Features(f) => self.features = f,
            CharacterChange::Abilities(a) => self.abilities = a,
            CharacterChange::Saves(s) => self.saves = s,
            CharacterChange::Skill(s, p) => self.skills[s.index()] = p,
            CharacterChange::JackOfAllTrades(j) => self.jack_of_all_trades = j,
            CharacterChange::Armor {
                formula,
                shield,
                bonus,
            } => {
                self.armor = formula;
                self.shield = shield;
                self.ac_bonus = bonus;
            }
            CharacterChange::InitiativeBonus(b) => self.initiative_bonus = b,
            CharacterChange::Speed(s) => self.speed = s,
            CharacterChange::Hp(hp) => self.hp = hp,
            CharacterChange::HitDiceSpent(n) => self.hit_dice_spent = n,
            CharacterChange::DeathSaves(ok, ko) => self.death_saves = (ok, ko),
            CharacterChange::Conditions(c) => self.conditions = c,
            CharacterChange::Exhaustion(e) => self.exhaustion = e,
            CharacterChange::Counters(c) => self.counters = c,
            CharacterChange::SlotsSpent(s) => self.slots_spent = s,
            CharacterChange::PactSpent(n) => self.pact_spent = n,
            CharacterChange::Spells(s) => self.spells = s,
            CharacterChange::Inventory(i) => self.inventory = i,
            CharacterChange::Currency(c) => self.currency = c,
            CharacterChange::Notes(n) => self.notes = n,
        }
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        let len = self.name.chars().count();
        let hp_ok = |v: i16| (-10_000..=10_000).contains(&v);
        let ok = (1..=MAX_NAME).contains(&len)
            && self.species.as_ref().is_none_or(link_ok)
            && self.background.as_ref().is_none_or(link_ok)
            && self.classes.len() <= 20
            && self.classes.iter().all(|c| {
                link_ok(&c.class)
                    && c.subclass.as_ref().is_none_or(link_ok)
                    && (1..=20).contains(&c.level)
                    && matches!(c.hit_die, 4 | 6 | 8 | 10 | 12)
            })
            && self.classes.iter().map(|c| u16::from(c.level)).sum::<u16>() <= 20
            && self.feats.len() <= MAX_LIST
            && self.feats.iter().all(link_ok)
            && self.features.len() <= MAX_LIST
            && self.features.iter().all(link_ok)
            && self.abilities.iter().all(|a| (1..=30).contains(a))
            && self.speed <= 1000
            && hp_ok(self.hp.current)
            && hp_ok(self.hp.max)
            && hp_ok(self.hp.temp)
            && self.hit_dice_spent <= 20
            && self.death_saves.0 <= 3
            && self.death_saves.1 <= 3
            && self.conditions.len() <= 30
            && self.conditions.iter().all(|c| text_ok(c))
            && self.exhaustion <= 10
            && self.counters.len() <= 50
            && self
                .counters
                .iter()
                .all(|c| text_ok(&c.name) && c.current <= c.max.max(c.current))
            && self.slots_spent.iter().all(|s| *s <= 20)
            && self.pact_spent <= 10
            && self.spells.len() <= MAX_INVENTORY
            && self
                .spells
                .iter()
                .all(|s| link_ok(&s.spell) && s.level <= 9)
            && self.inventory.len() <= MAX_INVENTORY
            && self
                .inventory
                .iter()
                .all(|i| link_ok(&i.item) && text_ok(&i.note))
            && self.notes.len() <= MAX_BODY_BYTES;
        if ok {
            Ok(())
        } else {
            Err(ValidationError::Length)
        }
    }

    pub fn summary(&self) -> CharacterSummary {
        CharacterSummary {
            id: self.id,
            owner: self.owner,
            name: self.name.clone(),
            species: self.species.as_ref().map(|s| s.name.clone()),
            classes: self
                .classes
                .iter()
                .map(|c| (c.class.name.clone(), c.level))
                .collect(),
            armor_class: self.armor_class(),
            hp: self.hp,
            active: self.active,
        }
    }

    pub fn level(&self) -> u8 {
        self.classes
            .iter()
            .map(|c| c.level)
            .fold(0u8, u8::saturating_add)
            .min(20)
    }

    pub fn proficiency_bonus(&self) -> i8 {
        proficiency_bonus(self.level().max(1))
    }

    pub fn modifier(&self, a: Ability) -> i8 {
        modifier(self.abilities[a.index()])
    }

    pub fn save(&self, a: Ability) -> i8 {
        let pb = if self.saves[a.index()] {
            self.proficiency_bonus()
        } else {
            0
        };
        self.modifier(a) + pb
    }

    pub fn skill(&self, s: Skill) -> i8 {
        self.modifier(s.ability())
            + proficiency_part(
                self.skills[s.index()],
                self.proficiency_bonus(),
                self.jack_of_all_trades,
            )
    }

    pub fn passive(&self, s: Skill) -> i8 {
        10 + self.skill(s)
    }

    pub fn initiative(&self) -> i8 {
        let jack = if self.jack_of_all_trades {
            self.proficiency_bonus() / 2
        } else {
            0
        };
        self.modifier(Ability::Dex) + jack + self.initiative_bonus
    }

    pub fn armor_class(&self) -> i16 {
        armor_class(self.armor, self.modifier(Ability::Dex), |a| {
            self.modifier(a)
        }) + if self.shield { 2 } else { 0 }
            + i16::from(self.ac_bonus)
    }

    pub fn d20_penalty(&self) -> i8 {
        rules::d20_penalty(self.rules, self.exhaustion)
    }

    pub fn spell_save_dc(&self, a: Ability) -> i8 {
        8 + self.proficiency_bonus() + self.modifier(a)
    }

    pub fn spell_attack(&self, a: Ability) -> i8 {
        self.proficiency_bonus() + self.modifier(a)
    }

    pub fn spell_slots(&self) -> [u8; 9] {
        let classes: Vec<(Caster, u8)> = self.classes.iter().map(|c| (c.caster, c.level)).collect();
        rules::spell_slots(&classes, self.rules)
    }

    pub fn pact_slots(&self) -> Option<(u8, u8)> {
        let level = self
            .classes
            .iter()
            .filter(|c| c.caster == Caster::Pact)
            .map(|c| c.level)
            .sum();
        rules::pact_slots(level)
    }

    pub fn hit_dice(&self) -> Vec<u8> {
        self.classes
            .iter()
            .flat_map(|c| std::iter::repeat_n(c.hit_die, usize::from(c.level)))
            .collect()
    }

    pub fn average_max_hp(&self) -> i16 {
        rules::average_hp(&self.hit_dice(), self.modifier(Ability::Con))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class(name: &str, level: u8, hit_die: u8, caster: Caster) -> ClassLevel {
        ClassLevel {
            class: Link {
                name: name.into(),
                ..Link::default()
            },
            subclass: None,
            level,
            hit_die,
            caster,
            spell_ability: None,
        }
    }

    fn hero() -> Character {
        let mut c = Character::new(
            CharacterId::from_bytes([1; 16]),
            UserId::from_bytes([2; 16]),
            "Ilsa".into(),
            RulesVersion::R2024,
        );
        c.abilities = [8, 16, 14, 10, 12, 17];
        c.classes = vec![
            class("Paladin", 2, 10, Caster::Half),
            class("Sorcerer", 3, 6, Caster::Full),
        ];
        c.saves[Ability::Wis.index()] = true;
        c.skills[Skill::Persuasion.index()] = Proficiency::Expertise;
        c.skills[Skill::Stealth.index()] = Proficiency::Proficient;
        c
    }

    fn one_saved(events: Vec<CampaignEvent>) -> Character {
        match events.as_slice() {
            [CampaignEvent::CharacterSaved { character }] => (**character).clone(),
            other => panic!("expected one saved character, got {other:?}"),
        }
    }

    #[test]
    fn ops_create_change_activate_and_delete() {
        let me = UserId::from_bytes([2; 16]);
        let first = CharacterOp::Create {
            id: CharacterId::from_bytes([1; 16]),
            name: " Ilsa ".into(),
            rules: RulesVersion::R2024,
        };
        let a = one_saved(first.events(me, &[]).unwrap());
        assert!(a.active && a.name == "Ilsa" && a.owner == me);
        let second = CharacterOp::Create {
            id: CharacterId::from_bytes([3; 16]),
            name: "Bran".into(),
            rules: RulesVersion::R2014,
        };
        let party = vec![a.clone()];
        let b = one_saved(second.events(me, &party).unwrap());
        assert!(!b.active, "only the first character starts active");
        let party = vec![a.clone(), b.clone()];
        let switched = CharacterOp::SetActive { id: b.id }
            .events(me, &party)
            .unwrap();
        assert_eq!(switched.len(), 2, "the old one turns off, the new one on");
        let hurt = CharacterOp::Change {
            id: a.id,
            change: CharacterChange::Hp(HitPoints {
                current: 3,
                max: 12,
                temp: 0,
            }),
        };
        let c = one_saved(hurt.events(me, &party).unwrap());
        assert_eq!(c.hp.current, 3);
        assert_eq!(c.summary().hp.max, 12);
        let bad = CharacterOp::Change {
            id: a.id,
            change: CharacterChange::Abilities([0; 6]),
        };
        assert!(bad.events(me, &party).is_err());
        assert!(first.events(me, &party).is_err(), "the id is taken");
        assert!(
            CharacterOp::Delete {
                id: CharacterId::from_bytes([9; 16])
            }
            .events(me, &party)
            .is_err()
        );
    }

    #[test]
    fn derived_values() {
        let c = hero();
        assert_eq!(c.level(), 5);
        assert_eq!(c.proficiency_bonus(), 3);
        assert_eq!(c.save(Ability::Wis), 4);
        assert_eq!(c.save(Ability::Str), -1);
        assert_eq!(c.skill(Skill::Persuasion), 3 + 6);
        assert_eq!(c.skill(Skill::Stealth), 3 + 3);
        assert_eq!(c.passive(Skill::Perception), 11);
        assert_eq!(c.initiative(), 3);
        assert_eq!(c.spell_save_dc(Ability::Cha), 14);
        assert_eq!(c.spell_attack(Ability::Cha), 6);
        assert_eq!(c.spell_slots(), rules::full_caster_slots(4));
        assert_eq!(c.pact_slots(), None);
        assert_eq!(c.hit_dice(), [10, 10, 6, 6, 6]);
        assert_eq!(c.average_max_hp(), 12 + 8 + 6 + 6 + 6);
    }

    #[test]
    fn armor_and_exhaustion() {
        let mut c = hero();
        assert_eq!(c.armor_class(), 13);
        c.armor = ArmorFormula::Armor {
            base: 16,
            dex_cap: Some(0),
        };
        c.shield = true;
        c.ac_bonus = 1;
        assert_eq!(c.armor_class(), 19);
        c.exhaustion = 2;
        assert_eq!(c.d20_penalty(), -4);
        c.rules = RulesVersion::R2014;
        assert_eq!(c.d20_penalty(), 0);
    }

    #[test]
    fn levels_one_to_twenty_and_beyond() {
        let mut c = hero();
        for level in 1..=20u8 {
            c.classes = vec![class("Wizard", level, 6, Caster::Full)];
            assert_eq!(c.level(), level);
            assert_eq!(c.spell_slots(), rules::full_caster_slots(level));
            assert_eq!(c.proficiency_bonus(), 2 + (level as i8 - 1) / 4);
        }
        c.classes = vec![
            class("Warlock", 15, 8, Caster::Pact),
            class("Bard", 10, 8, Caster::Full),
        ];
        assert_eq!(c.level(), 20, "the total is capped at 20");
        assert_eq!(c.pact_slots(), Some((3, 5)));
        c.classes.clear();
        assert_eq!(c.proficiency_bonus(), 2);
    }
}
