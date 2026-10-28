use serde::{Deserialize, Serialize};

use crate::compendium::RulesVersion;
use crate::ids::{CharacterId, UserId};
use crate::rules::{
    self, Ability, ArmorFormula, Caster, Proficiency, Skill, armor_class, modifier,
    proficiency_bonus, proficiency_part,
};

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
