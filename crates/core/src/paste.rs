use serde::{Deserialize, Serialize};

use crate::compendium::{ChallengeRating, Facets, Kind};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pasted {
    pub name: String,
    pub facets: Facets,
    pub text: String,
    pub complete: bool,
}

const SIZES: [&str; 6] = ["tiny", "small", "medium", "large", "huge", "gargantuan"];

const SCHOOLS: [&str; 8] = [
    "abjuration",
    "conjuration",
    "divination",
    "enchantment",
    "evocation",
    "illusion",
    "necromancy",
    "transmutation",
];

const RARITIES: [&str; 7] = [
    "very rare",
    "uncommon",
    "common",
    "rare",
    "legendary",
    "artifact",
    "rarity varies",
];

const ITEM_TYPES: [&str; 10] = [
    "wondrous item",
    "armor",
    "weapon",
    "potion",
    "ring",
    "rod",
    "scroll",
    "staff",
    "wand",
    "ammunition",
];

const CLASSES: [&str; 13] = [
    "Artificer",
    "Barbarian",
    "Bard",
    "Cleric",
    "Druid",
    "Fighter",
    "Monk",
    "Paladin",
    "Ranger",
    "Rogue",
    "Sorcerer",
    "Warlock",
    "Wizard",
];

const SECTIONS: [&str; 8] = [
    "Traits",
    "Actions",
    "Bonus Actions",
    "Reactions",
    "Legendary Actions",
    "Lair Actions",
    "Mythic Actions",
    "Regional Effects",
];

const MONSTER_STATS: [&str; 22] = [
    "Armor Class",
    "AC",
    "Hit Points",
    "HP",
    "Speed",
    "Initiative",
    "Saving Throws",
    "Saves",
    "Skills",
    "Damage Vulnerabilities",
    "Damage Resistances",
    "Damage Immunities",
    "Vulnerabilities",
    "Resistances",
    "Immunities",
    "Condition Immunities",
    "Senses",
    "Languages",
    "Challenge",
    "CR",
    "Gear",
    "Proficiency Bonus",
];

const SPELL_STATS: [&str; 5] = ["Casting Time", "Range", "Components", "Duration", "Classes"];

const ABILITIES: [&str; 6] = ["STR", "DEX", "CON", "INT", "WIS", "CHA"];

const CONNECTORS: [&str; 11] = [
    "of", "the", "and", "a", "an", "to", "in", "on", "with", "for", "or",
];

struct Header {
    facets: Facets,
    name: Option<String>,
    lines: usize,
}

pub fn parse(text: &str, default: Kind) -> Vec<Pasted> {
    let clean = text.replace(['\u{a0}', '\r'], " ");
    let lines: Vec<&str> = clean
        .lines()
        .map(|l| l.trim_matches(' '))
        .filter(|l| !l.trim().is_empty())
        .collect();
    let starts: Vec<(usize, Header)> = (0..lines.len())
        .filter_map(|i| entry_start(&lines, i).map(|h| (i, h)))
        .collect();
    let first = starts.first().map_or(lines.len(), |(i, _)| *i);
    let mut out = Vec::new();
    let preamble = &lines[..first];
    if (starts.is_empty() || preamble.iter().map(|l| l.len()).sum::<usize>() > 200)
        && let Some(p) = fallback(preamble, default)
    {
        out.push(p);
    }
    for (k, (start, header)) in starts.iter().enumerate() {
        let end = starts.get(k + 1).map_or(lines.len(), |(i, _)| *i);
        out.push(entry(&lines[*start..end], header));
    }
    out
}

fn entry_start(lines: &[&str], i: usize) -> Option<Header> {
    let title = lines[i];
    if !is_title(title) {
        return None;
    }
    if let Some(h) = subclass_title(title) {
        return Some(h);
    }
    let next = lines.get(i + 1)?;
    let after = lines.get(i + 2).copied();
    let facets = spell(next)
        .or_else(|| monster(next))
        .or_else(|| item(next, after))
        .or_else(|| feat(next))
        .or_else(|| subclass_line(next))?;
    let lines = match facets {
        Facets::MagicItem { .. } if after.is_some_and(|a| a.starts_with("Rarity ")) => 2,
        _ => 1,
    };
    Some(Header {
        facets,
        name: None,
        lines,
    })
}

fn is_title(l: &str) -> bool {
    let words = l.split_whitespace().count();
    l.chars().count() <= 80
        && (1..=10).contains(&words)
        && l.starts_with(|c: char| c.is_uppercase() || c.is_ascii_digit())
        && !l.ends_with(['.', ':', ',', ';'])
        && !l.contains(". ")
        && !l.contains('\t')
}

fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn spell(line: &str) -> Option<Facets> {
    let lower = line.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| c.is_whitespace() || c == '(')
        .filter(|w| !w.is_empty())
        .collect();
    let school_at = |w: &str| SCHOOLS.iter().find(|s| w == **s).map(|s| title_case(s));
    let (level, school) = match words.as_slice() {
        ["level", n, s, ..] => (n.parse::<u8>().ok()?, school_at(s)?),
        [s, "cantrip", ..] => (0, school_at(s)?),
        ["cantrip", s, ..] => (0, school_at(s)?),
        [first, s, ..] => {
            let n = first.strip_suffix("-level")?;
            let digits: String = n.chars().take_while(char::is_ascii_digit).collect();
            (digits.parse::<u8>().ok()?, school_at(s)?)
        }
        _ => return None,
    };
    if level > 9 {
        return None;
    }
    let classes = line
        .rfind('(')
        .and_then(|o| line[o + 1..].strip_suffix(')'))
        .filter(|c| !c.eq_ignore_ascii_case("ritual"))
        .map(|c| {
            c.split(',')
                .map(|x| x.trim().to_owned())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default();
    Some(Facets::Spell {
        level,
        school,
        classes,
        ritual: lower.contains("ritual"),
        concentration: false,
    })
}

fn monster(line: &str) -> Option<Facets> {
    let (first, rest) = line.split_once(' ')?;
    let size = SIZES.iter().find(|s| first.eq_ignore_ascii_case(s))?;
    let (kind_part, _) = rest.split_once(',')?;
    let (size, kind_part) = match kind_part
        .strip_prefix("or ")
        .and_then(|r| r.split_once(' '))
    {
        Some((other, t)) if SIZES.iter().any(|s| other.eq_ignore_ascii_case(s)) => (
            format!(
                "{} or {}",
                title_case(size),
                title_case(&other.to_lowercase())
            ),
            t,
        ),
        _ => (title_case(size), kind_part),
    };
    let creature = kind_part.split('(').next()?.trim();
    if creature.is_empty() || creature.split_whitespace().count() > 5 {
        return None;
    }
    Some(Facets::Monster {
        cr: ChallengeRating::from_eighths(0),
        creature_type: title_case(creature),
        size,
    })
}

fn find_cr(block: &[&str]) -> Option<ChallengeRating> {
    block.iter().find_map(|l| {
        l.strip_prefix("CR ")
            .or_else(|| l.strip_prefix("Challenge "))
            .and_then(|r| r.split_whitespace().next())
            .and_then(parse_cr)
    })
}

pub fn parse_cr(s: &str) -> Option<ChallengeRating> {
    let eighths = match s.split_once('/') {
        Some((n, d)) => {
            let (n, d): (u16, u16) = (n.parse().ok()?, d.parse().ok()?);
            (d != 0 && 8 % d == 0).then(|| n * (8 / d))?
        }
        None => s.parse::<u16>().ok()?.checked_mul(8)?,
    };
    (eighths <= 320).then(|| ChallengeRating::from_eighths(eighths))
}

fn without_attunement(s: &str) -> String {
    let lower = s.to_lowercase();
    match lower.find("(requires attunement") {
        Some(i) => {
            let end = lower[i..].find(')').map_or(s.len(), |j| i + j + 1);
            format!("{}{}", &s[..i], &s[end..])
        }
        None => s.to_owned(),
    }
}

fn rarity_in(text: &str) -> Option<String> {
    let text = without_attunement(text);
    let text = text.trim().trim_end_matches(',').trim();
    let lower = text.to_lowercase();
    let single = RARITIES.iter().find(|r| lower == **r);
    match single {
        Some(r) => Some(title_case(r)),
        None => RARITIES
            .iter()
            .any(|r| lower.starts_with(r))
            .then(|| text.to_owned()),
    }
}

fn after_type(line: &str) -> &str {
    let mut depth = 0;
    for (i, c) in line.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => return line[i + 1..].trim(),
            _ => {}
        }
    }
    ""
}

fn item(line: &str, after: Option<&str>) -> Option<Facets> {
    let lower = line.to_lowercase();
    if !ITEM_TYPES.iter().any(|t| lower.starts_with(t)) {
        return None;
    }
    let rarity = match after.and_then(|a| a.strip_prefix("Rarity ")) {
        Some(r) => rarity_in(r)?,
        None => rarity_in(after_type(line))?,
    };
    let category = line.split([',', '(']).next()?.trim();
    Some(Facets::MagicItem {
        rarity,
        category: category.to_owned(),
        attunement: lower.contains("requires attunement")
            || after.is_some_and(|a| a.to_lowercase().contains("requires attunement")),
    })
}

fn feat(line: &str) -> Option<Facets> {
    if line.starts_with("Prerequisite:") {
        return Some(Facets::Feat {
            category: "General".into(),
        });
    }
    let head = line.split(" (").next()?;
    let category = head.strip_suffix(" Feat")?;
    (!category.is_empty() && category.split_whitespace().count() <= 3).then(|| Facets::Feat {
        category: category.to_owned(),
    })
}

fn class_name(s: &str) -> Option<String> {
    CLASSES
        .iter()
        .find(|c| s.eq_ignore_ascii_case(c))
        .map(|c| (*c).to_owned())
}

fn subclass_line(line: &str) -> Option<Facets> {
    let lower = line.to_lowercase();
    let (class, _) = lower.split_once(" subclass")?;
    Some(Facets::Subclass {
        class: class_name(class)?,
    })
}

fn subclass_title(title: &str) -> Option<Header> {
    let (left, name) = title.split_once(':')?;
    let class = class_name(left.trim().trim_end_matches(" Subclass"))?;
    let name = name.trim();
    (!name.is_empty()).then(|| Header {
        facets: Facets::Subclass { class },
        name: Some(name.to_owned()),
        lines: 0,
    })
}

fn entry(block: &[&str], header: &Header) -> Pasted {
    let name = header.name.clone().unwrap_or_else(|| block[0].to_owned());
    let head = &block[1..=header.lines];
    let body = &block[1 + header.lines..];
    let mut facets = header.facets.clone();
    let mut complete = true;
    match &mut facets {
        Facets::Spell { concentration, .. } => {
            *concentration = body.iter().any(|l| {
                l.strip_prefix("Duration")
                    .map(|r| r.trim_start_matches([':', ' ']))
                    .is_some_and(|r| r.starts_with("Concentration"))
            });
        }
        Facets::MagicItem { attunement, .. } => {
            *attunement |= body
                .first()
                .is_some_and(|l| l.to_lowercase().starts_with("requires attunement"));
        }
        Facets::Monster { cr, .. } => match find_cr(body) {
            Some(found) => *cr = found,
            None => complete = false,
        },
        _ => {}
    }
    let mut blocks: Vec<String> = head.iter().map(|h| format!("*{h}*")).collect();
    blocks.extend(format_body(body, facets.kind()));
    Pasted {
        name,
        facets,
        text: blocks.join("\n\n"),
        complete,
    }
}

fn fallback(lines: &[&str], kind: Kind) -> Option<Pasted> {
    let (name, body) = match lines.split_first()? {
        (first, rest) if is_title(first) => ((*first).to_owned(), rest),
        _ => {
            let words: Vec<&str> = lines[0].split_whitespace().take(6).collect();
            (words.join(" "), lines)
        }
    };
    let (facets, complete) = default_facets(kind);
    Some(Pasted {
        name,
        facets,
        text: format_body(body, kind).join("\n\n"),
        complete,
    })
}

pub fn default_facets(kind: Kind) -> (Facets, bool) {
    let unknown = || "Unknown".to_owned();
    match kind {
        Kind::Spell => (
            Facets::Spell {
                level: 0,
                school: unknown(),
                classes: Vec::new(),
                ritual: false,
                concentration: false,
            },
            false,
        ),
        Kind::Monster => (
            Facets::Monster {
                cr: ChallengeRating::from_eighths(0),
                creature_type: unknown(),
                size: "Medium".into(),
            },
            false,
        ),
        Kind::MagicItem => (
            Facets::MagicItem {
                rarity: unknown(),
                category: unknown(),
                attunement: false,
            },
            false,
        ),
        Kind::Equipment => (
            Facets::Equipment {
                category: unknown(),
            },
            false,
        ),
        Kind::Feat => (
            Facets::Feat {
                category: unknown(),
            },
            false,
        ),
        Kind::Subclass => (Facets::Subclass { class: unknown() }, false),
        Kind::Feature => (
            Facets::Feature {
                class: unknown(),
                level: 1,
            },
            false,
        ),
        Kind::Class => (Facets::Class, true),
        Kind::Species => (Facets::Species, true),
        Kind::Background => (Facets::Background, true),
        Kind::Trait => (Facets::Trait, true),
        Kind::Condition => (Facets::Condition, true),
    }
}

fn stat_label<'a>(line: &'a str, labels: &[&str]) -> Option<(&'a str, &'a str)> {
    labels.iter().find_map(|l| {
        let rest = line.strip_prefix(l)?;
        let rest = rest.strip_prefix(':').unwrap_or(rest);
        rest.starts_with([' ', '\t'])
            .then(|| (&line[..l.len()], rest.trim()))
    })
}

fn bullet(line: &str) -> Option<&str> {
    ["• ", "◦ ", "▪ ", "- ", "* ", "•"]
        .iter()
        .find_map(|b| line.strip_prefix(b))
        .map(str::trim)
}

fn inline_label(line: &str) -> Option<(&str, &str)> {
    let i = line.find(". ")?;
    let label = &line[..i];
    if label.chars().count() > 60 || !label.starts_with(|c: char| c.is_uppercase()) {
        return None;
    }
    let mut depth = 0;
    let mut words = 0;
    for w in label.split_whitespace() {
        let open = w.starts_with('(');
        if open {
            depth += 1;
        }
        if depth == 0 {
            words += 1;
            let ok = w.starts_with(|c: char| c.is_uppercase() || c.is_ascii_digit())
                || CONNECTORS.contains(&w);
            if !ok {
                return None;
            }
        }
        if w.ends_with(')') {
            depth = 0;
        }
    }
    (words <= 8).then(|| (label, &line[i + 2..]))
}

fn abilities(lines: &[&str]) -> Option<(String, usize)> {
    let first = lines.first()?;
    if !first
        .split_whitespace()
        .next()
        .is_some_and(|w| w.eq_ignore_ascii_case("STR"))
    {
        return None;
    }
    let mut toks: Vec<(usize, &str)> = Vec::new();
    for (li, l) in lines.iter().enumerate().take(24) {
        toks.extend(l.split_whitespace().map(|t| (li, t)));
    }
    let is_ab = |t: &str, k: usize| t.eq_ignore_ascii_case(ABILITIES[k]);
    let mut pos = Vec::with_capacity(6);
    let mut from = 0;
    for k in 0..6 {
        let p = (from..toks.len()).find(|&j| is_ab(toks[j].1, k))?;
        pos.push(p);
        from = p + 1;
    }
    let adjacent = pos.windows(2).all(|w| w[1] == w[0] + 1);
    let mut cursor = if adjacent { pos[5] + 1 } else { 0 };
    let mut values = Vec::with_capacity(6);
    for p in &pos {
        if !adjacent {
            cursor = p + 1;
        }
        let score = toks.get(cursor)?.1;
        if !score.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        cursor += 1;
        let mut value = score.to_owned();
        if let Some(&(_, m)) = toks.get(cursor) {
            let signed = m.starts_with(['+', '-', '−', '–']);
            if m.starts_with('(') {
                value.push_str(&format!(" {m}"));
                cursor += 1;
            } else if signed {
                value.push_str(&format!(" ({m})"));
                cursor += 1;
                if toks
                    .get(cursor)
                    .is_some_and(|(_, s)| s.starts_with(['+', '-', '−', '–']))
                {
                    cursor += 1;
                }
            }
        }
        values.push(value);
    }
    let used = toks.get(cursor - 1).map_or(0, |t| t.0) + 1;
    let table = format!(
        "| {} |\n|{}\n| {} |",
        ABILITIES.join(" | "),
        " --- |".repeat(6),
        values.join(" | ")
    );
    Some((table, used))
}

fn tab_table(lines: &[&str]) -> Option<(String, usize)> {
    let n = lines.iter().take_while(|l| l.contains('\t')).count();
    if n < 2 {
        return None;
    }
    let rows: Vec<Vec<String>> = lines[..n]
        .iter()
        .map(|l| {
            l.split('\t')
                .map(|c| c.trim().replace('|', "\\|"))
                .collect()
        })
        .collect();
    let width = rows.iter().map(Vec::len).max().unwrap_or(1);
    let line = |r: &Vec<String>| {
        let mut cells = r.clone();
        cells.resize(width, String::new());
        format!("| {} |", cells.join(" | "))
    };
    let mut out = vec![line(&rows[0]), format!("|{}", " --- |".repeat(width))];
    out.extend(rows[1..].iter().map(line));
    Some((out.join("\n"), n))
}

fn format_body(lines: &[&str], kind: Kind) -> Vec<String> {
    let stats: &[&str] = match kind {
        Kind::Monster => &MONSTER_STATS,
        Kind::Spell => &SPELL_STATS,
        _ => &["Prerequisite", "Prerequisites"],
    };
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some((table, used)) = abilities(&lines[i..]).or_else(|| tab_table(&lines[i..])) {
            out.push(table);
            i += used;
            continue;
        }
        if stat_label(line, stats).is_some() {
            let mut group = Vec::new();
            while let Some((label, value)) = lines.get(i).and_then(|l| stat_label(l, stats)) {
                let colon = if kind == Kind::Monster { "" } else { ":" };
                group.push(format!("**{label}{colon}** {value}"));
                i += 1;
            }
            out.push(group.join("  \n"));
            continue;
        }
        if let Some(s) = SECTIONS.iter().find(|s| line.eq_ignore_ascii_case(s)) {
            out.push(format!("## {s}"));
            i += 1;
            continue;
        }
        if bullet(line).is_some() {
            let mut group = Vec::new();
            while let Some(b) = lines.get(i).and_then(|l| bullet(l)) {
                group.push(format!("- {b}"));
                i += 1;
            }
            out.push(group.join("\n"));
            continue;
        }
        if let Some((label, rest)) = inline_label(line) {
            out.push(format!("**{label}.** {rest}"));
        } else if is_title(line)
            && line.split_whitespace().count() <= 8
            && lines.get(i + 1).is_some_and(|n| !is_title(n))
        {
            out.push(format!("### {line}"));
        } else {
            out.push(line.to_owned());
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(text: &str, default: Kind) -> Pasted {
        let mut v = parse(text, default);
        assert_eq!(v.len(), 1, "{v:#?}");
        v.remove(0)
    }

    #[test]
    fn spell_2024_and_2014() {
        let p = one(
            "Fireball\nLevel 3 Evocation (Sorcerer, Wizard)\nCasting Time: Action\nRange: 150 feet\nComponents: V, S, M (a ball of bat guano)\nDuration: Instantaneous\nA bright streak flashes from you.\nUsing a Higher-Level Spell Slot. The damage increases by 1d6.",
            Kind::Trait,
        );
        assert_eq!(p.name, "Fireball");
        assert_eq!(
            p.facets,
            Facets::Spell {
                level: 3,
                school: "Evocation".into(),
                classes: vec!["Sorcerer".into(), "Wizard".into()],
                ritual: false,
                concentration: false
            }
        );
        assert_eq!(
            p.text,
            "*Level 3 Evocation (Sorcerer, Wizard)*\n\n**Casting Time:** Action  \n**Range:** 150 feet  \n**Components:** V, S, M (a ball of bat guano)  \n**Duration:** Instantaneous\n\nA bright streak flashes from you.\n\n**Using a Higher-Level Spell Slot.** The damage increases by 1d6."
        );
        let p = one(
            "Detect Magic\n1st-level divination (ritual)\nCasting Time: 1 action\nDuration: Concentration, up to 10 minutes\nFor the duration, you sense magic.",
            Kind::Trait,
        );
        assert!(matches!(
            p.facets,
            Facets::Spell { level: 1, ritual: true, concentration: true, ref school, .. } if school == "Divination"
        ));
        let p = one(
            "Fire Bolt\nEvocation Cantrip (Sorcerer, Wizard)\nYou hurl a mote of fire.",
            Kind::Trait,
        );
        assert!(matches!(p.facets, Facets::Spell { level: 0, .. }));
    }

    #[test]
    fn monster_stat_block() {
        let p = one(
            "Goblin Warrior\nSmall Fey (Goblinoid), Chaotic Neutral\nAC 15\nHP 10 (3d6)\nSpeed 30 ft.\nSTR\nDEX\nCON\nINT\nWIS\nCHA\n8 (−1)\n15 (+2)\n10 (+0)\n10 (+0)\n8 (−1)\n8 (−1)\nSenses Darkvision 60 ft.\nCR 1/4 (XP 50; PB +2)\nActions\nScimitar. Melee Attack Roll: +4, reach 5 ft. Hit: 5 (1d6 + 2) Slashing damage.\nBonus Actions\nNimble Escape. The goblin takes the Disengage or Hide action.",
            Kind::Trait,
        );
        assert_eq!(
            p.facets,
            Facets::Monster {
                cr: ChallengeRating::from_eighths(2),
                creature_type: "Fey".into(),
                size: "Small".into()
            }
        );
        assert!(p.complete);
        assert!(p.text.starts_with("*Small Fey (Goblinoid), Chaotic Neutral*\n\n**AC** 15  \n**HP** 10 (3d6)  \n**Speed** 30 ft.\n\n| STR | DEX | CON | INT | WIS | CHA |\n| --- | --- | --- | --- | --- | --- |\n| 8 (−1) | 15 (+2) | 10 (+0) | 10 (+0) | 8 (−1) | 8 (−1) |\n\n**Senses** Darkvision 60 ft.  \n**CR** 1/4 (XP 50; PB +2)\n\n## Actions\n\n**Scimitar.** Melee Attack Roll"), "{}", p.text);
        assert!(p.text.ends_with("## Bonus Actions\n\n**Nimble Escape.** The goblin takes the Disengage or Hide action."), "{}", p.text);
        let p = one(
            "Ogre\nLarge giant, chaotic evil\nArmor Class 11 (hide armor)\nSTR 19 (+4) DEX 8 (-1) CON 16 (+3) INT 5 (-3) WIS 7 (-2) CHA 7 (-2)\nChallenge 2 (450 XP)",
            Kind::Trait,
        );
        assert!(
            matches!(p.facets, Facets::Monster { cr, ref creature_type, .. } if cr.eighths() == 16 && creature_type == "Giant")
        );
        assert!(
            p.text
                .contains("| 19 (+4) | 8 (-1) | 16 (+3) | 5 (-3) | 7 (-2) | 7 (-2) |"),
            "{}",
            p.text
        );
    }

    #[test]
    fn items_feats_subclasses() {
        let p = one(
            "Flame Tongue\nWeapon (Any Simple or Martial), Rare (Requires Attunement)\nWhile holding this magic weapon, you can take a Bonus Action.",
            Kind::Trait,
        );
        assert_eq!(
            p.facets,
            Facets::MagicItem {
                rarity: "Rare".into(),
                category: "Weapon".into(),
                attunement: true
            }
        );
        let p = one(
            "Alert\nOrigin Feat\nYou gain the following benefits.\nInitiative Proficiency. When you roll Initiative, add your Proficiency Bonus.",
            Kind::Trait,
        );
        assert_eq!(
            p.facets,
            Facets::Feat {
                category: "Origin".into()
            }
        );
        assert!(p.text.ends_with(
            "**Initiative Proficiency.** When you roll Initiative, add your Proficiency Bonus."
        ));
        let p = one(
            "Sorcerer: Storm Sorcery\nYour innate magic comes from elemental air.\nWind Speaker\nThe arcane magic you command is infused with elemental air.\nHeart of the Storm\nAt 6th level, you gain resistance.",
            Kind::Trait,
        );
        assert_eq!(p.name, "Storm Sorcery");
        assert_eq!(
            p.facets,
            Facets::Subclass {
                class: "Sorcerer".into()
            }
        );
        assert_eq!(
            p.text,
            "Your innate magic comes from elemental air.\n\n### Wind Speaker\n\nThe arcane magic you command is infused with elemental air.\n\n### Heart of the Storm\n\nAt 6th level, you gain resistance."
        );
    }

    #[test]
    fn bulk_paste_splits_entries_and_keeps_unknown_text() {
        let text = "Chapter 7: Spells\nThis chapter describes the spells, with a long introduction that is more than two hundred characters long so that it is kept as its own entry rather than being dropped from the paste, because it might matter to the user who pasted it.\nAcid Splash\nEvocation Cantrip (Sorcerer, Wizard)\nYou create an acidic bubble.\nAid\nLevel 2 Abjuration (Bard, Cleric)\nYour spell bolsters your allies.";
        let v = parse(text, Kind::Condition);
        let names: Vec<&str> = v.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Chapter 7: Spells", "Acid Splash", "Aid"]);
        assert_eq!(v[0].facets, Facets::Condition);
        let v = parse("Just some notes\nwithout any header at all.", Kind::Spell);
        assert_eq!(v.len(), 1);
        assert!(!v[0].complete);
        assert_eq!(v[0].name, "Just some notes");
    }

    #[test]
    fn bullets_and_tab_tables() {
        let v = parse(
            "Rules\nIntro line here.\n• First point\n• Second point\nd4\tEffect\n1\tNothing\n2\tSomething",
            Kind::Trait,
        );
        assert_eq!(
            v[0].text,
            "Intro line here.\n\n- First point\n- Second point\n\n| d4 | Effect |\n| --- | --- |\n| 1 | Nothing |\n| 2 | Something |"
        );
    }
}
