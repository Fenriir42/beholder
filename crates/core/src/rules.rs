use serde::{Deserialize, Serialize};

use crate::compendium::RulesVersion;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ability {
    Str,
    Dex,
    Con,
    Int,
    Wis,
    Cha,
}

impl Ability {
    pub const ALL: [Self; 6] = [
        Self::Str,
        Self::Dex,
        Self::Con,
        Self::Int,
        Self::Wis,
        Self::Cha,
    ];

    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Skill {
    Acrobatics,
    AnimalHandling,
    Arcana,
    Athletics,
    Deception,
    History,
    Insight,
    Intimidation,
    Investigation,
    Medicine,
    Nature,
    Perception,
    Performance,
    Persuasion,
    Religion,
    SleightOfHand,
    Stealth,
    Survival,
}

impl Skill {
    pub const ALL: [Self; 18] = [
        Self::Acrobatics,
        Self::AnimalHandling,
        Self::Arcana,
        Self::Athletics,
        Self::Deception,
        Self::History,
        Self::Insight,
        Self::Intimidation,
        Self::Investigation,
        Self::Medicine,
        Self::Nature,
        Self::Perception,
        Self::Performance,
        Self::Persuasion,
        Self::Religion,
        Self::SleightOfHand,
        Self::Stealth,
        Self::Survival,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn ability(self) -> Ability {
        match self {
            Self::Athletics => Ability::Str,
            Self::Acrobatics | Self::SleightOfHand | Self::Stealth => Ability::Dex,
            Self::Arcana | Self::History | Self::Investigation | Self::Nature | Self::Religion => {
                Ability::Int
            }
            Self::AnimalHandling
            | Self::Insight
            | Self::Medicine
            | Self::Perception
            | Self::Survival => Ability::Wis,
            Self::Deception | Self::Intimidation | Self::Performance | Self::Persuasion => {
                Ability::Cha
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Proficiency {
    #[default]
    None,
    Proficient,
    Expertise,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Caster {
    #[default]
    None,
    Full,
    Half,
    HalfRoundedUp,
    Third,
    Pact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArmorFormula {
    Unarmored,
    Armor { base: u8, dex_cap: Option<u8> },
    UnarmoredDefense { plus: Ability },
    Natural { base: u8 },
    Fixed { ac: u8 },
}

pub fn class_defaults(name: &str) -> Option<(u8, Caster, Option<Ability>)> {
    let d = match name.trim().to_lowercase().as_str() {
        "barbarian" => (12, Caster::None, None),
        "bard" => (8, Caster::Full, Some(Ability::Cha)),
        "cleric" => (8, Caster::Full, Some(Ability::Wis)),
        "druid" => (8, Caster::Full, Some(Ability::Wis)),
        "fighter" => (10, Caster::None, None),
        "monk" => (8, Caster::None, None),
        "paladin" => (10, Caster::Half, Some(Ability::Cha)),
        "ranger" => (10, Caster::Half, Some(Ability::Wis)),
        "rogue" => (8, Caster::None, None),
        "sorcerer" => (6, Caster::Full, Some(Ability::Cha)),
        "warlock" => (8, Caster::Pact, Some(Ability::Cha)),
        "wizard" => (6, Caster::Full, Some(Ability::Int)),
        "artificer" => (8, Caster::HalfRoundedUp, Some(Ability::Int)),
        _ => return None,
    };
    Some(d)
}

pub fn modifier(score: u8) -> i8 {
    ((i16::from(score) - 10).div_euclid(2)) as i8
}

pub fn proficiency_bonus(total_level: u8) -> i8 {
    2 + (total_level.clamp(1, 20) as i8 - 1) / 4
}

pub fn proficiency_part(p: Proficiency, pb: i8, jack_of_all_trades: bool) -> i8 {
    match p {
        Proficiency::Expertise => pb * 2,
        Proficiency::Proficient => pb,
        Proficiency::None if jack_of_all_trades => pb / 2,
        Proficiency::None => 0,
    }
}

pub fn d20_penalty(rules: RulesVersion, exhaustion: u8) -> i8 {
    match rules {
        RulesVersion::R2024 => -2 * exhaustion.min(6) as i8,
        RulesVersion::R2014 | RulesVersion::LevelUp => 0,
    }
}

pub fn armor_class(formula: ArmorFormula, dex: i8, other: impl Fn(Ability) -> i8) -> i16 {
    let dex = i16::from(dex);
    match formula {
        ArmorFormula::Unarmored => 10 + dex,
        ArmorFormula::Armor { base, dex_cap } => {
            i16::from(base) + dex_cap.map_or(dex, |cap| dex.min(i16::from(cap)))
        }
        ArmorFormula::UnarmoredDefense { plus } => 10 + dex + i16::from(other(plus)),
        ArmorFormula::Natural { base } => i16::from(base) + dex,
        ArmorFormula::Fixed { ac } => i16::from(ac),
    }
}

const FULL_SLOTS: [[u8; 9]; 20] = [
    [2, 0, 0, 0, 0, 0, 0, 0, 0],
    [3, 0, 0, 0, 0, 0, 0, 0, 0],
    [4, 2, 0, 0, 0, 0, 0, 0, 0],
    [4, 3, 0, 0, 0, 0, 0, 0, 0],
    [4, 3, 2, 0, 0, 0, 0, 0, 0],
    [4, 3, 3, 0, 0, 0, 0, 0, 0],
    [4, 3, 3, 1, 0, 0, 0, 0, 0],
    [4, 3, 3, 2, 0, 0, 0, 0, 0],
    [4, 3, 3, 3, 1, 0, 0, 0, 0],
    [4, 3, 3, 3, 2, 0, 0, 0, 0],
    [4, 3, 3, 3, 2, 1, 0, 0, 0],
    [4, 3, 3, 3, 2, 1, 0, 0, 0],
    [4, 3, 3, 3, 2, 1, 1, 0, 0],
    [4, 3, 3, 3, 2, 1, 1, 0, 0],
    [4, 3, 3, 3, 2, 1, 1, 1, 0],
    [4, 3, 3, 3, 2, 1, 1, 1, 0],
    [4, 3, 3, 3, 2, 1, 1, 1, 1],
    [4, 3, 3, 3, 3, 1, 1, 1, 1],
    [4, 3, 3, 3, 3, 2, 1, 1, 1],
    [4, 3, 3, 3, 3, 2, 2, 1, 1],
];

pub fn full_caster_slots(caster_level: u8) -> [u8; 9] {
    match caster_level {
        0 => [0; 9],
        l => FULL_SLOTS[usize::from(l.min(20)) - 1],
    }
}

fn own_caster_level(caster: Caster, level: u8, rules: RulesVersion) -> u8 {
    match caster {
        Caster::Full => level,
        Caster::Half if rules == RulesVersion::R2014 && level < 2 => 0,
        Caster::Half | Caster::HalfRoundedUp => level.div_ceil(2),
        Caster::Third if level < 3 => 0,
        Caster::Third => level.div_ceil(3),
        Caster::None | Caster::Pact => 0,
    }
}

fn multiclass_share(caster: Caster, level: u8, rules: RulesVersion) -> u8 {
    match caster {
        Caster::Full => level,
        Caster::Half if rules == RulesVersion::R2014 => level / 2,
        Caster::Half | Caster::HalfRoundedUp => level.div_ceil(2),
        Caster::Third => level / 3,
        Caster::None | Caster::Pact => 0,
    }
}

pub fn spell_slots(classes: &[(Caster, u8)], rules: RulesVersion) -> [u8; 9] {
    let casters: Vec<&(Caster, u8)> = classes
        .iter()
        .filter(|(c, _)| !matches!(c, Caster::None | Caster::Pact))
        .collect();
    let caster_level = match casters.as_slice() {
        [] => 0,
        [(c, l)] => own_caster_level(*c, *l, rules),
        many => many
            .iter()
            .map(|(c, l)| multiclass_share(*c, *l, rules))
            .sum(),
    };
    full_caster_slots(caster_level)
}

pub fn pact_slots(warlock_level: u8) -> Option<(u8, u8)> {
    let (count, level) = match warlock_level {
        0 => return None,
        1 => (1, 1),
        2 => (2, 1),
        3..=4 => (2, 2),
        5..=6 => (2, 3),
        7..=8 => (2, 4),
        9..=10 => (2, 5),
        11..=16 => (3, 5),
        _ => (4, 5),
    };
    Some((count, level))
}

pub fn average_hp(hit_dice: &[u8], con: i8) -> i16 {
    hit_dice
        .iter()
        .enumerate()
        .map(|(i, &die)| {
            let roll = if i == 0 {
                i16::from(die)
            } else {
                i16::from(die) / 2 + 1
            };
            (roll + i16::from(con)).max(1)
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_and_proficiency() {
        let mods: Vec<i8> = [1, 3, 8, 9, 10, 11, 12, 15, 20, 30]
            .into_iter()
            .map(modifier)
            .collect();
        assert_eq!(mods, [-5, -4, -1, -1, 0, 0, 1, 2, 5, 10]);
        let pb: Vec<i8> = (1..=20).map(proficiency_bonus).collect();
        assert_eq!(
            pb,
            [2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6]
        );
        assert_eq!(proficiency_part(Proficiency::Expertise, 3, false), 6);
        assert_eq!(proficiency_part(Proficiency::None, 3, true), 1);
        assert_eq!(proficiency_part(Proficiency::None, 3, false), 0);
        assert_eq!(d20_penalty(RulesVersion::R2024, 3), -6);
        assert_eq!(d20_penalty(RulesVersion::R2014, 3), 0);
    }

    #[test]
    fn armor_formulas() {
        let wis = |a| if a == Ability::Wis { 3 } else { 0 };
        assert_eq!(armor_class(ArmorFormula::Unarmored, 2, wis), 12);
        let half_plate = ArmorFormula::Armor {
            base: 15,
            dex_cap: Some(2),
        };
        assert_eq!(armor_class(half_plate, 4, wis), 17);
        let plate = ArmorFormula::Armor {
            base: 18,
            dex_cap: Some(0),
        };
        assert_eq!(armor_class(plate, 3, wis), 18);
        let leather = ArmorFormula::Armor {
            base: 11,
            dex_cap: None,
        };
        assert_eq!(armor_class(leather, 4, wis), 15);
        assert_eq!(
            armor_class(
                ArmorFormula::UnarmoredDefense { plus: Ability::Wis },
                3,
                wis
            ),
            16
        );
        assert_eq!(armor_class(ArmorFormula::Natural { base: 13 }, 2, wis), 15);
        assert_eq!(armor_class(ArmorFormula::Fixed { ac: 19 }, 5, wis), 19);
    }

    #[test]
    fn single_class_slots_for_every_level() {
        let r = RulesVersion::R2014;
        for level in 1..=20u8 {
            assert_eq!(
                spell_slots(&[(Caster::Full, level)], r),
                full_caster_slots(level)
            );
        }
        let total = |s: [u8; 9]| s.iter().map(|&n| u32::from(n)).sum::<u32>();
        assert_eq!(
            total(spell_slots(&[(Caster::Half, 1)], RulesVersion::R2014)),
            0
        );
        assert_eq!(
            spell_slots(&[(Caster::Half, 1)], RulesVersion::R2024)[0],
            2,
            "2024 paladins and rangers cast from level 1"
        );
        assert_eq!(
            spell_slots(&[(Caster::Half, 5)], r),
            [4, 2, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            spell_slots(&[(Caster::Half, 20)], r),
            [4, 3, 3, 3, 2, 0, 0, 0, 0]
        );
        assert_eq!(spell_slots(&[(Caster::HalfRoundedUp, 1)], r)[0], 2);
        assert_eq!(total(spell_slots(&[(Caster::Third, 2)], r)), 0);
        assert_eq!(spell_slots(&[(Caster::Third, 3)], r)[0], 2);
        assert_eq!(
            spell_slots(&[(Caster::Third, 7)], r),
            [4, 2, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            spell_slots(&[(Caster::Third, 20)], r),
            [4, 3, 3, 1, 0, 0, 0, 0, 0]
        );
        assert_eq!(spell_slots(&[(Caster::Pact, 5)], r), [0; 9]);
    }

    #[test]
    fn multiclass_slots() {
        let paladin_sorcerer = [(Caster::Half, 3), (Caster::Full, 3)];
        assert_eq!(
            spell_slots(&paladin_sorcerer, RulesVersion::R2014),
            full_caster_slots(4)
        );
        assert_eq!(
            spell_slots(&paladin_sorcerer, RulesVersion::R2024),
            full_caster_slots(5),
            "2024 rounds half casters up"
        );
        let fighter_wizard = [(Caster::None, 5), (Caster::Full, 2)];
        assert_eq!(
            spell_slots(&fighter_wizard, RulesVersion::R2014),
            full_caster_slots(2),
            "one spellcasting class uses its own table"
        );
        let ek_cleric = [(Caster::Third, 4), (Caster::Full, 1)];
        assert_eq!(
            spell_slots(&ek_cleric, RulesVersion::R2014),
            full_caster_slots(2)
        );
        let warlock_bard = [(Caster::Pact, 5), (Caster::Full, 2)];
        assert_eq!(
            spell_slots(&warlock_bard, RulesVersion::R2014),
            full_caster_slots(2)
        );
    }

    #[test]
    fn standard_class_defaults() {
        assert_eq!(
            class_defaults(" Wizard "),
            Some((6, Caster::Full, Some(Ability::Int)))
        );
        assert_eq!(
            class_defaults("warlock"),
            Some((8, Caster::Pact, Some(Ability::Cha)))
        );
        assert_eq!(class_defaults("Barbarian"), Some((12, Caster::None, None)));
        assert_eq!(class_defaults("Blood Hunter"), None);
    }

    #[test]
    fn pact_magic_and_hp() {
        assert_eq!(pact_slots(0), None);
        assert_eq!(pact_slots(1), Some((1, 1)));
        assert_eq!(pact_slots(5), Some((2, 3)));
        assert_eq!(pact_slots(11), Some((3, 5)));
        assert_eq!(pact_slots(20), Some((4, 5)));
        assert_eq!(average_hp(&[10, 10, 10], 2), 12 + 8 + 8);
        assert_eq!(average_hp(&[6], -3), 3);
        assert_eq!(average_hp(&[6, 6], -5), 1 + 1);
    }
}
