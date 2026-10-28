use core::fmt;

use serde::{Deserialize, Serialize};

use crate::character::Character;
use crate::rules::Ability;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Roll {
    pub count: u8,
    pub sides: u8,
    pub bonus: i16,
}

impl fmt::Display for Roll {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.count, self.bonus) {
            (0, b) => write!(f, "{b}"),
            (n, 0) => write!(f, "{n}d{}", self.sides),
            (n, b) if b > 0 => write!(f, "{n}d{}+{b}", self.sides),
            (n, b) => write!(f, "{n}d{}{b}", self.sides),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Check {
    Attack,
    Save(Ability),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpellInfo {
    pub casting_time: Option<String>,
    pub range: Option<String>,
    pub duration: Option<String>,
    pub components: Option<String>,
    pub check: Option<Check>,
    pub half_on_save: bool,
    pub damage: Option<Roll>,
    pub damage_type: Option<String>,
    pub damage_adds_modifier: bool,
    pub healing: Option<Roll>,
    pub adds_modifier: bool,
    pub cantrip_scaling: bool,
    pub upcast: Option<Roll>,
    pub upcast_above: Option<u8>,
    pub projectiles: u8,
    pub extra_projectile_per_level: bool,
    pub cantrip_projectiles: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Weapon {
    pub damage: Roll,
    pub versatile: Option<Roll>,
    pub damage_type: String,
    pub finesse: bool,
    pub ranged: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Details {
    pub spell: Option<SpellInfo>,
    pub weapon: Option<Weapon>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resolution {
    Attack {
        bonus: i8,
    },
    Save {
        ability: Ability,
        dc: i8,
        half: bool,
    },
    Automatic,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effect {
    pub resolution: Resolution,
    pub damage: Option<Roll>,
    pub damage_type: Option<String>,
    pub healing: Option<Roll>,
    pub times: u8,
}

const DAMAGE_TYPES: [&str; 13] = [
    "acid",
    "bludgeoning",
    "cold",
    "fire",
    "force",
    "lightning",
    "necrotic",
    "piercing",
    "poison",
    "psychic",
    "radiant",
    "slashing",
    "thunder",
];

const ABILITY_NAMES: [(&str, Ability); 6] = [
    ("strength", Ability::Str),
    ("dexterity", Ability::Dex),
    ("constitution", Ability::Con),
    ("intelligence", Ability::Int),
    ("wisdom", Ability::Wis),
    ("charisma", Ability::Cha),
];

const NUMBERS: [(&str, u8); 9] = [
    ("two", 2),
    ("three", 3),
    ("four", 4),
    ("five", 5),
    ("six", 6),
    ("seven", 7),
    ("eight", 8),
    ("nine", 9),
    ("ten", 10),
];

fn dice(t: &str) -> Option<Roll> {
    let t = t.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    let (n, m) = t.split_once('d')?;
    let count = if n.is_empty() { 1 } else { n.parse().ok()? };
    let sides = m.parse().ok()?;
    (count > 0 && sides > 0).then_some(Roll {
        count,
        sides,
        bonus: 0,
    })
}

fn words(text: &str) -> Vec<String> {
    text.replace(['*', '|', '(', ')', ',', ':', ';'], " ")
        .split_whitespace()
        .map(|w| w.trim_end_matches('.').to_lowercase())
        .collect()
}

fn field(text: &str, label: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let clean = line.replace(['*', '|'], " ");
        let clean = clean.trim();
        let rest = clean.strip_prefix(label).or_else(|| {
            clean
                .get(..label.len())
                .filter(|p| p.eq_ignore_ascii_case(label))
                .map(|_| &clean[label.len()..])
        })?;
        let value = rest.trim_start_matches([':', ' ']).trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

fn find_seq(w: &[String], seq: &[&str]) -> Option<usize> {
    (0..w.len().saturating_sub(seq.len() - 1))
        .find(|&i| seq.iter().enumerate().all(|(k, s)| w[i + k] == *s))
}

const SPELL_MODIFIER: [&str; 4] = ["your", "spellcasting", "ability", "modifier"];

fn modifier_at(w: &[String], j: usize) -> Option<usize> {
    let plus = matches!(w.get(j).map(String::as_str), Some("+" | "plus"));
    let rest = w.get(j + 1..j + 5)?;
    (plus && rest.iter().zip(SPELL_MODIFIER).all(|(a, b)| a == b)).then_some(j + 5)
}

struct Found {
    roll: Roll,
    kind: Option<String>,
    adds_modifier: bool,
}

fn damage_after(w: &[String], from: usize) -> Option<Found> {
    for i in from..w.len() {
        let Some(mut roll) = dice(&w[i]) else {
            continue;
        };
        let mut j = i + 1;
        let mut adds_modifier = false;
        if let Some(after) = modifier_at(w, j) {
            adds_modifier = true;
            j = after;
        } else if w.get(j).map(String::as_str) == Some("+")
            && let Some(b) = w.get(j + 1).and_then(|b| b.parse::<i16>().ok())
        {
            roll.bonus = b;
            j += 2;
        }
        let before = &w[i.saturating_sub(4)..i];
        let equal_to = before.len() >= 2
            && before[before.len() - 2] == "equal"
            && before[before.len() - 1] == "to";
        let window = &w[j..w.len().min(j + 3)];
        let kind = window
            .iter()
            .chain(before.iter().filter(|_| equal_to))
            .find(|x| DAMAGE_TYPES.contains(&x.as_str()))
            .cloned();
        let damage_word = window.iter().any(|x| x == "damage")
            || (equal_to && before.iter().any(|x| x == "damage"));
        if kind.is_some() || damage_word {
            return Some(Found {
                roll,
                kind,
                adds_modifier,
            });
        }
    }
    None
}

fn ordinal(w: &str) -> Option<u8> {
    w.trim_end_matches(|c: char| c.is_ascii_alphabetic())
        .parse()
        .ok()
}

pub fn parse_spell(text: &str) -> SpellInfo {
    let w = words(text);
    let attack = w
        .windows(2)
        .position(|p| p[0] == "spell" && p[1] == "attack");
    let save = w.windows(3).position(|p| {
        ABILITY_NAMES.iter().any(|(n, _)| p[0] == *n) && p[1] == "saving" && p[2] == "throw"
    });
    let save_check = |s: usize| {
        ABILITY_NAMES
            .iter()
            .find(|(n, _)| w[s] == *n)
            .map(|(_, a)| Check::Save(*a))
    };
    let header_save = find_seq(&w, &["saving", "throw"])
        .and_then(|i| w.get(i + 2))
        .and_then(|a| ABILITY_NAMES.iter().find(|(n, _)| a == n))
        .map(|(_, a)| Check::Save(*a));
    let check = match (attack, save) {
        (Some(a), Some(s)) if s < a => save_check(s),
        (Some(_), _) => Some(Check::Attack),
        (None, Some(s)) => save_check(s),
        (None, None) => header_save,
    };
    let higher = find_seq(&w, &["higher", "levels"])
        .or_else(|| find_seq(&w, &["higher-level", "spell", "slot"]))
        .or_else(|| find_seq(&w, &["of", "higher", "level"]))
        .or_else(|| {
            (0..w.len().saturating_sub(1)).find(|&i| {
                w[i] == "or"
                    && w[i + 1] == "higher"
                    && w[i + 2..w.len().min(i + 30)]
                        .iter()
                        .any(|x| x == "increases" || x == "each")
            })
        })
        .map(|h| {
            w[..h]
                .iter()
                .rposition(|x| x == "when" || x == "at")
                .filter(|&k| h - k < 12)
                .unwrap_or(h)
        });
    let healing_at = find_seq(&w, &["regains"]).or_else(|| find_seq(&w, &["regain"]));
    let healing = healing_at.and_then(|h| w[h..w.len().min(h + 14)].iter().find_map(|x| dice(x)));
    let body_end = higher.unwrap_or(w.len());
    let damage = if healing.is_some() {
        None
    } else {
        damage_after(&w[..body_end], 0)
    };
    let upcast = higher.and_then(|h| {
        let by = find_seq(&w[h..], &["increases", "by"])? + h + 2;
        dice(w.get(by)?)
    });
    let upcast_above = higher.and_then(|h| {
        let at = find_seq(&w[h..], &["level", "above"])
            .or_else(|| find_seq(&w[h..], &["rank", "above"]))
            .or_else(|| find_seq(&w[h..], &["ranks", "above"]))?
            + h
            + 2;
        let n = if w.get(at).map(String::as_str) == Some("rank") {
            at + 1
        } else {
            at
        };
        ordinal(w.get(n)?)
    });
    let cantrip_scaling = find_seq(&w, &["reach", "5th", "level"]).is_some()
        || find_seq(&w, &["reaches", "5th", "level"]).is_some()
        || find_seq(&w, &["reach", "levels", "5"]).is_some();
    let projectile_words = [
        "dart", "darts", "beam", "beams", "ray", "rays", "missiles", "bolts",
    ];
    let projectiles = (0..body_end)
        .find_map(|i| {
            let n = NUMBERS.iter().find(|(name, _)| w[i] == *name)?.1;
            w[i + 1..w.len().min(i + 3)]
                .iter()
                .any(|x| projectile_words.contains(&x.as_str()))
                .then_some(n)
        })
        .unwrap_or(1);
    let extra_projectile_per_level = higher.is_some_and(|h| {
        w[h..]
            .windows(2)
            .any(|p| p[0] == "more" && projectile_words.contains(&p[1].as_str()))
    });
    let cantrip_projectiles = find_seq(&w, &["two", "beams", "at", "level", "5"]).is_some()
        || find_seq(&w, &["two", "beams", "at", "5th", "level"]).is_some();
    SpellInfo {
        casting_time: field(text, "Casting Time"),
        range: field(text, "Range"),
        duration: field(text, "Duration"),
        components: field(text, "Components"),
        check,
        half_on_save: find_seq(&w, &["half", "as", "much", "damage"]).is_some(),
        damage: damage.as_ref().map(|f| f.roll),
        damage_adds_modifier: damage.as_ref().is_some_and(|f| f.adds_modifier),
        damage_type: damage.and_then(|f| f.kind).or_else(|| {
            find_seq(&w, &["damage", "type"])
                .and_then(|i| w.get(i + 2))
                .filter(|t| DAMAGE_TYPES.contains(&t.as_str()))
                .cloned()
        }),
        healing,
        adds_modifier: healing.is_some()
            && find_seq(&w, &["plus", "your", "spellcasting", "ability", "modifier"]).is_some(),
        cantrip_scaling,
        upcast,
        upcast_above,
        projectiles,
        extra_projectile_per_level,
        cantrip_projectiles,
    }
}

pub fn parse_weapon(text: &str) -> Option<Weapon> {
    let w = words(text);
    let at = w.iter().position(|x| x == "damage")?;
    let dice_at = (at + 1..w.len().min(at + 4)).find(|&i| dice(&w[i]).is_some())?;
    let damage = dice(&w[dice_at])?;
    let versatile = w.get(dice_at + 1).and_then(|x| dice(x)).or_else(|| {
        let two = w.iter().position(|x| x == "two-handed")?;
        w.get(two + 1).and_then(|x| dice(x))
    });
    let damage_type = w[dice_at..]
        .iter()
        .take(12)
        .find(|x| DAMAGE_TYPES.contains(&x.as_str()))
        .cloned()
        .unwrap_or_default();
    let all = w.join(" ");
    Some(Weapon {
        damage,
        versatile,
        damage_type,
        finesse: all.contains("finesse"),
        ranged: all.contains("ranged weapon") && !all.contains("melee weapon"),
    })
}

fn tier(level: u8) -> u8 {
    1 + u8::from(level >= 5) + u8::from(level >= 11) + u8::from(level >= 17)
}

pub fn spell_effect(
    ch: &Character,
    info: &SpellInfo,
    spell_level: u8,
    cast_level: u8,
    ability: Ability,
) -> Effect {
    let resolution = match info.check {
        Some(Check::Attack) => Resolution::Attack {
            bonus: ch.spell_attack(ability),
        },
        Some(Check::Save(a)) => Resolution::Save {
            ability: a,
            dc: ch.spell_save_dc(ability),
            half: info.half_on_save,
        },
        None => Resolution::Automatic,
    };
    let above = info.upcast_above.unwrap_or(spell_level);
    let extra = cast_level.saturating_sub(above.max(spell_level));
    let scale = |base: Roll| {
        let mut r = base;
        if spell_level == 0 && info.cantrip_scaling {
            r.count = r.count.saturating_mul(tier(ch.level().max(1)));
        }
        if let Some(up) = info.upcast.filter(|u| u.sides == r.sides) {
            r.count = r.count.saturating_add(up.count.saturating_mul(extra));
        }
        r
    };
    let healing = info.healing.map(|h| {
        let mut r = scale(h);
        if info.adds_modifier {
            r.bonus += i16::from(ch.modifier(ability));
        }
        r
    });
    let mut times = info.projectiles.max(1);
    if info.extra_projectile_per_level {
        times = times.saturating_add(extra);
    }
    if spell_level == 0 && info.cantrip_projectiles {
        times = tier(ch.level().max(1));
    }
    Effect {
        resolution,
        damage: info.damage.map(|d| {
            let mut r = scale(d);
            if info.damage_adds_modifier {
                r.bonus += i16::from(ch.modifier(ability));
            }
            r
        }),
        damage_type: info.damage_type.clone(),
        healing,
        times,
    }
}

pub fn weapon_effect(ch: &Character, w: &Weapon, two_handed: bool) -> Effect {
    let (str_mod, dex_mod) = (ch.modifier(Ability::Str), ch.modifier(Ability::Dex));
    let m = if w.ranged {
        dex_mod
    } else if w.finesse {
        str_mod.max(dex_mod)
    } else {
        str_mod
    };
    let base = if two_handed {
        w.versatile.unwrap_or(w.damage)
    } else {
        w.damage
    };
    Effect {
        resolution: Resolution::Attack {
            bonus: m + ch.proficiency_bonus(),
        },
        damage: Some(Roll {
            bonus: base.bonus + i16::from(m),
            ..base
        }),
        damage_type: (!w.damage_type.is_empty()).then(|| w.damage_type.clone()),
        healing: None,
        times: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::{ClassLevel, Link};
    use crate::compendium::RulesVersion;
    use crate::ids::{CharacterId, UserId};
    use crate::rules::Caster;

    const FIREBALL: &str = "*Level 3 Evocation* (Sorcerer, Wizard)\n\n| | |\n|---|---|\n| Casting Time | Action |\n| Range | 150 feet |\n| Components | V, S, M (a ball of bat guano and sulfur) |\n| Duration | Instantaneous |\n\nA bright streak flashes from you to a point you choose within range and then blossoms with a low roar into a fiery explosion. Each creature in a 20-foot-radius Sphere centered on that point makes a Dexterity saving throw, taking 8d6 Fire damage on a failed save or half as much damage on a successful one.\n\n**Using a Higher-Level Spell Slot.** The damage increases by 1d6 for each spell slot level above 3.";
    const FIRE_BOLT: &str = "| Casting Time | Action |\n| Range | 120 feet |\n\nYou hurl a mote of fire at a creature or an object within range. Make a ranged spell attack against the target. On a hit, the target takes 1d10 Fire damage.\n\nCantrip Upgrade. The damage increases by 1d10 when you reach levels 5 (2d10), 11 (3d10), and 17 (4d10).";
    const CURE: &str = "A creature you touch regains a number of Hit Points equal to 2d8 plus your spellcasting ability modifier. Using a Higher-Level Spell Slot. The healing increases by 2d8 for each spell slot level above 1.";
    const MISSILE: &str = "You create three glowing darts of magical force. Each dart strikes a creature of your choice that you can see within range. A dart deals 1d4 + 1 Force damage to its target.\n\n**Using a Higher-Level Spell Slot.** The spell creates one more dart for each spell slot level above 1.";
    const BLAST: &str = "Make a ranged spell attack against one creature or object in range. On a hit, the target takes 1d10 Force damage.\n\nCantrip Upgrade. The spell creates two beams at level 5, three beams at level 11, and four beams at level 17.";
    const LURE_2014: &str = "*Evocation Cantrip*\n\n**Casting Time:** 1 action\n\n**Range:** Self (15-foot radius)\n\n**Components:** V\n\n**Duration:** Instantaneous\n\nYou create a lash of lightning energy that strikes at one creature of your choice that you can see within 15 feet of you. The target must succeed on a Strength saving throw or be pulled up to 10 feet in a straight line toward you and then take 1d8 lightning damage if it is within 5 feet of you. This spell’s damage increases by 1d8 when you reach 5th level (2d8), 11th level (3d8), and 17th level (4d8).";
    const BOLT_2014: &str = "*3rd-Level Evocation*\n\n**Casting Time:** 1 action\n\n**Range:** Self (100-foot line)\n\nEach creature in the line must make a Dexterity saving throw. A creature takes 8d6 lightning damage on a failed save, or half as much damage on a successful one.\n\n**At Higher Levels.** When you cast this spell using a spell slot of 4th level or higher, the damage increases by 1d6 for each slot level above 3rd.";
    const HOLD: &str = "Choose a Humanoid that you can see within range. The target must succeed on a Wisdom saving throw or have the Paralyzed condition for the duration.";

    fn sorcerer(level: u8) -> Character {
        let mut c = Character::new(
            CharacterId::from_bytes([1; 16]),
            UserId::from_bytes([2; 16]),
            "Misaka".into(),
            RulesVersion::R2014,
        );
        c.abilities = [13, 18, 11, 13, 12, 16];
        c.classes = vec![ClassLevel {
            class: Link {
                name: "Sorcerer".into(),
                ..Link::default()
            },
            subclass: None,
            level,
            hit_die: 6,
            caster: Caster::Full,
            spell_ability: Some(Ability::Cha),
        }];
        c
    }

    #[test]
    fn rolls_display_like_dice_notation() {
        let r = |count, sides, bonus| {
            Roll {
                count,
                sides,
                bonus,
            }
            .to_string()
        };
        assert_eq!(r(8, 6, 0), "8d6");
        assert_eq!(r(1, 4, 1), "1d4+1");
        assert_eq!(r(1, 8, -1), "1d8-1");
        assert_eq!(r(0, 0, 5), "5");
    }

    #[test]
    fn spells_are_parsed_from_srd_and_converted_texts() {
        let f = parse_spell(FIREBALL);
        assert_eq!(f.check, Some(Check::Save(Ability::Dex)));
        assert!(f.half_on_save);
        assert_eq!(f.damage.map(|r| r.to_string()).as_deref(), Some("8d6"));
        assert_eq!(f.damage_type.as_deref(), Some("fire"));
        assert_eq!(f.upcast.map(|r| r.to_string()).as_deref(), Some("1d6"));
        assert_eq!(f.upcast_above, Some(3));
        assert_eq!(f.casting_time.as_deref(), Some("Action"));
        assert_eq!(f.range.as_deref(), Some("150 feet"));
        let b = parse_spell(FIRE_BOLT);
        assert_eq!(b.check, Some(Check::Attack));
        assert!(b.cantrip_scaling);
        let c = parse_spell(CURE);
        assert_eq!(c.healing.map(|r| r.to_string()).as_deref(), Some("2d8"));
        assert!(c.adds_modifier && c.damage.is_none());
        let m = parse_spell(MISSILE);
        assert_eq!(m.damage.map(|r| r.to_string()).as_deref(), Some("1d4+1"));
        assert_eq!(
            (m.projectiles, m.extra_projectile_per_level, m.check),
            (3, true, None)
        );
        assert!(parse_spell(BLAST).cantrip_projectiles);
        let l = parse_spell(LURE_2014);
        assert_eq!(l.check, Some(Check::Save(Ability::Str)));
        assert!(l.cantrip_scaling);
        assert_eq!(l.casting_time.as_deref(), Some("1 action"));
        assert_eq!(l.range.as_deref(), Some("Self (15-foot radius)"));
        let lb = parse_spell(BOLT_2014);
        assert_eq!(
            (lb.upcast_above, lb.damage_type.as_deref()),
            (Some(3), Some("lightning"))
        );
        let h = parse_spell(HOLD);
        assert_eq!((h.check, h.damage), (Some(Check::Save(Ability::Wis)), None));
    }

    #[test]
    fn other_phrasings_from_libraries() {
        let conjure = "Make a melee spell attack. On a hit, the target takes psychic damage equal to 3d12 plus your spellcasting ability modifier.";
        let c = parse_spell(conjure);
        assert_eq!(
            (
                c.damage.map(|r| r.to_string()),
                c.damage_type.as_deref(),
                c.damage_adds_modifier
            ),
            (Some("3d12".into()), Some("psychic"), true)
        );
        let shards =
            "A comet deals 2d4 + your spellcasting ability modifier force damage to its target.";
        let s = parse_spell(shards);
        assert_eq!(
            (s.damage_type.as_deref(), s.damage_adds_modifier),
            (Some("force"), true)
        );
        let effect = spell_effect(&sorcerer(5), &c, 2, 2, Ability::Cha);
        assert_eq!(
            effect.damage.map(|r| r.to_string()).as_deref(),
            Some("3d12+3")
        );
        let power = "Each creature must make a Dexterity saving throw, taking 3d6 cold damage on a failure. When you use this power at rank 2 or higher, the damage increases by 1d6 for each rank above 1.";
        let p = parse_spell(power);
        assert_eq!(
            (p.upcast.map(|r| r.to_string()), p.upcast_above),
            (Some("1d6".into()), Some(1))
        );
        let slot = "The target takes 4d8 fire damage. When you cast this spell using a spell slot of 3rd level or higher, the damage increases by 1d8 for each slot level above 2nd.";
        let sl = parse_spell(slot);
        assert_eq!(
            (
                sl.upcast.map(|r| r.to_string()),
                sl.upcast_above,
                sl.damage.map(|r| r.to_string())
            ),
            (Some("1d8".into()), Some(2), Some("4d8".into()))
        );
        let hinted = parse_spell(
            "**Saving Throw:** Constitution\n\n**Damage Type:** necrotic\n\nIt takes 2d8 damage on a failure.",
        );
        assert_eq!(
            (hinted.check, hinted.damage_type.as_deref()),
            (Some(Check::Save(Ability::Con)), Some("necrotic"))
        );
        let tap = "The target takes 1d8 necrotic damage. The damage of this spell increases by 1d8 when the caster reaches 5th level (2d8), 11th level (3d8), and 17th level (4d8).";
        assert!(parse_spell(tap).cantrip_scaling);
    }

    #[test]
    fn effects_follow_the_character() {
        let ch = sorcerer(5);
        let bolt = spell_effect(&ch, &parse_spell(FIRE_BOLT), 0, 0, Ability::Cha);
        assert_eq!(bolt.resolution, Resolution::Attack { bonus: 6 });
        assert_eq!(bolt.damage.map(|r| r.to_string()).as_deref(), Some("2d10"));
        let fb = parse_spell(FIREBALL);
        let at3 = spell_effect(&ch, &fb, 3, 3, Ability::Cha);
        assert_eq!(
            at3.resolution,
            Resolution::Save {
                ability: Ability::Dex,
                dc: 14,
                half: true
            }
        );
        assert_eq!(at3.damage.map(|r| r.to_string()).as_deref(), Some("8d6"));
        let at5 = spell_effect(&ch, &fb, 3, 5, Ability::Cha);
        assert_eq!(at5.damage.map(|r| r.to_string()).as_deref(), Some("10d6"));
        let cure = spell_effect(&ch, &parse_spell(CURE), 1, 2, Ability::Cha);
        assert_eq!(
            cure.healing.map(|r| r.to_string()).as_deref(),
            Some("4d8+3")
        );
        let missile = spell_effect(&ch, &parse_spell(MISSILE), 1, 3, Ability::Cha);
        assert_eq!(
            (missile.times, missile.resolution),
            (5, Resolution::Automatic)
        );
        let blast = spell_effect(&sorcerer(11), &parse_spell(BLAST), 0, 0, Ability::Cha);
        assert_eq!(blast.times, 3);
        assert_eq!(blast.damage.map(|r| r.to_string()).as_deref(), Some("1d10"));
    }

    #[test]
    fn weapons_from_srd_and_converted_texts() {
        let rapier = "*Weapons*\n\n| | |\n|---|---|\n| Type | Martial Melee Weapons, Martial Weapons, Melee Weapons |\n| Damage | 1d8 Piercing |\n| Properties | Finesse |";
        let w = parse_weapon(rapier).unwrap();
        assert_eq!(
            (
                w.damage.to_string(),
                w.damage_type.as_str(),
                w.finesse,
                w.ranged
            ),
            ("1d8".into(), "piercing", true, false)
        );
        let staff = "| Type | Melee Weapons, Simple Weapons |\n| Damage | 1d6 Bludgeoning |\n| Two-Handed | 1d8 Bludgeoning |\n| Properties | Versatile |";
        assert_eq!(
            parse_weapon(staff)
                .unwrap()
                .versatile
                .map(|r| r.to_string())
                .as_deref(),
            Some("1d8")
        );
        let bow = "| Type | Martial Ranged Weapons, Martial Weapons, Ranged Weapons |\n| Damage | 1d8 Piercing |";
        assert!(parse_weapon(bow).unwrap().ranged);
        let converted = "*Melee Weapon*\n\nLongsword Melee weapon (martial, longsword) Versatile. Damage: 1d8 (1d10) Damage Type: Slashing Properties:";
        let w = parse_weapon(converted).unwrap();
        assert_eq!(
            (
                w.damage.to_string(),
                w.versatile.map(|r| r.to_string()),
                w.damage_type.as_str()
            ),
            ("1d8".into(), Some("1d10".into()), "slashing")
        );
        let essentials = "*Melee Weapon*\n\n**Dagger Melee weapon (simple, dagger) Damage.** 1d4 Damage Type: \n\n**Piercing Properties.** \n\n**Finesse, Light, Range, Thrown Range.** 20/60";
        let w = parse_weapon(essentials).unwrap();
        assert_eq!(
            (w.damage.to_string(), w.damage_type.as_str(), w.finesse),
            ("1d4".into(), "piercing", true)
        );
        assert_eq!(parse_weapon("A shiny ring that glows."), None);
        let ch = sorcerer(5);
        let e = weapon_effect(&ch, &parse_weapon(rapier).unwrap(), false);
        assert_eq!(e.resolution, Resolution::Attack { bonus: 7 });
        assert_eq!(e.damage.map(|r| r.to_string()).as_deref(), Some("1d8+4"));
    }
}
