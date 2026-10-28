use std::path::Path;

use beholder_core::compendium::{ChallengeRating, Facets, Kind, RulesVersion};
use beholder_core::paste::{default_facets, parse_cr};
use beholder_core::{NoteBody, NoteTitle};
use serde_json::{Map, Value, json};

mod tables;

use tables as dnd_tables;

pub const FILES: [&str; 6] = [
    "spells",
    "monsters",
    "items",
    "classes",
    "species",
    "backgrounds",
];

const SECTIONS: [&str; 7] = [
    "Legendary Actions",
    "Mythic Actions",
    "Bonus Actions",
    "Lair Actions",
    "Reactions",
    "Actions",
    "Traits",
];

const RARITIES: [&str; 7] = [
    "common",
    "uncommon",
    "rare",
    "very rare",
    "legendary",
    "artifact",
    "varies",
];

pub struct Converted {
    pub file: String,
    pub entries: usize,
    pub json: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}: {1}")]
    Json(String, serde_json::Error),
}

pub fn convert_all(dir: &Path) -> Result<Vec<Converted>, Error> {
    let mut out = Vec::new();
    for name in FILES {
        let path = dir.join(format!("{name}.json"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut rows: Vec<Value> =
            serde_json::from_str(&text).map_err(|e| Error::Json(path.display().to_string(), e))?;
        rows.iter_mut().for_each(numbers_as_text);
        let entries: Vec<Value> = rows.iter().flat_map(|r| convert(name, r)).collect();
        let n = entries.len();
        let file = json!({ "format": "beholder-library", "version": 1, "entries": entries });
        out.push(Converted {
            file: format!("{name}.json"),
            entries: n,
            json: serde_json::to_string_pretty(&file)
                .map_err(|e| Error::Json(name.to_owned(), e))?,
        });
    }
    Ok(out)
}

pub fn numbers_as_text(r: &mut Value) {
    if let Some(props) = r["properties"].as_object_mut() {
        for v in props.values_mut() {
            if let Value::Number(n) = v {
                *v = Value::String(n.to_string());
            }
        }
    }
}

fn prop<'a>(r: &'a Value, key: &str) -> Option<&'a str> {
    r["properties"][key]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn title(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| {
                    f.to_uppercase()
                        .chain(c.flat_map(char::to_lowercase))
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect::<Vec<String>>()
        .join(" ")
}

fn short(s: &str, max: usize) -> String {
    s.chars().take(max).collect::<String>().trim().to_owned()
}

fn rules_of(book: &str) -> &'static str {
    let b = book.to_lowercase();
    if b.contains("level up") || b.contains("a5e") {
        "level-up"
    } else if b.contains("2024") {
        "2024"
    } else {
        "2014"
    }
}

pub fn convert(file: &str, r: &Value) -> Vec<Value> {
    let Some(entry) = convert_one(file, r) else {
        return Vec::new();
    };
    if file == "classes" {
        return split_free_subclass(entry)
            .into_iter()
            .map(|mut e| {
                if let Some(o) = e.as_object_mut() {
                    o.remove("source_text");
                }
                e
            })
            .collect();
    }
    vec![entry]
}

fn convert_one(file: &str, r: &Value) -> Option<Value> {
    let cleaned: String = r["name"]
        .as_str()?
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let name = short(
        &cleaned.split_whitespace().collect::<Vec<_>>().join(" "),
        120,
    );
    if name.is_empty() {
        return None;
    }
    let cleaned_description = clean_description(r["description"].as_str().unwrap_or_default());
    let description = cleaned_description.trim();
    let book = r["book"]
        .as_str()
        .map(|b| short(b.trim(), 120))
        .filter(|b| !b.is_empty());
    let mut e = Map::new();
    e.insert("name".into(), json!(name));
    if let Some(b) = &book {
        e.insert("book".into(), json!(b));
        e.insert("rules".into(), json!(rules_of(b)));
    }
    let (kind, text) = match file {
        "spells" => spell(r, description, &mut e),
        "monsters" => monster(r, description, &mut e),
        "items" => item(r, description, &mut e),
        "classes" => ("class", class_text(description)),
        "species" => ("species", species_text(description)),
        _ => ("background", background_text(description)),
    };
    e.insert("kind".into(), json!(kind));
    e.insert("text".into(), json!(truncate_bytes(&text, 64 * 1024)));
    if file == "classes" {
        e.insert("source_text".into(), json!(description));
    }
    Some(Value::Object(e))
}

fn truncate_bytes(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_owned();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_owned()
}

fn spell(r: &Value, raw: &str, e: &mut Map<String, Value>) -> (&'static str, String) {
    let mut normalized = raw
        .replace("Rang e:", "Range:")
        .replace("Casting time", "Casting Time");
    for label in SPELL_LABELS {
        normalized = normalized.replace(&format!("{label} :"), &format!("{label}:"));
    }
    let description = normalized.as_str();
    let head: String = description.chars().take(160).collect();
    let head_lower = head.to_lowercase();
    let level = prop(r, "Level")
        .and_then(|l| {
            if l.eq_ignore_ascii_case("cantrip") {
                Some(0)
            } else {
                l.parse::<u8>().ok()
            }
        })
        .or_else(|| {
            if head_lower.contains("cantrip") {
                return Some(0);
            }
            head.chars()
                .find(char::is_ascii_digit)?
                .to_digit(10)
                .map(|d| d as u8)
        })
        .unwrap_or(0)
        .min(9);
    let school = prop(r, "School")
        .map(title)
        .unwrap_or_else(|| "Unknown".into());
    let classes: Vec<String> = prop(r, "Classes")
        .map(str::to_owned)
        .or_else(|| {
            let before = head.split("Casting Time").next()?;
            let open = before.find('(')?;
            let close = before[open..].find(')')? + open;
            Some(before[open + 1..close].to_owned())
        })
        .map(|c| {
            c.split(',')
                .map(|x| short(&title(x.trim()), 80))
                .filter(|x| !x.is_empty())
                .take(32)
                .collect()
        })
        .unwrap_or_default();
    let casting = prop(r, "Casting Time").unwrap_or_default().to_lowercase();
    let ritual = casting.contains("ritual") || head_lower.contains("ritual");
    let concentration = description.contains("Duration: Concentration")
        || prop(r, "Duration").is_some_and(|d| d.to_lowercase().contains("concentration"));
    e.insert("level".into(), json!(level));
    e.insert("school".into(), json!(short(&school, 80)));
    if !classes.is_empty() {
        e.insert("classes".into(), json!(classes));
    }
    if ritual {
        e.insert("ritual".into(), json!(true));
    }
    if concentration {
        e.insert("concentration".into(), json!(true));
    }
    let text = spell_text(description);
    let missing: Vec<String> = [
        ("Casting Time", "Casting Time"),
        ("Range", "data-RangeAoe"),
        ("Components", "Components"),
    ]
    .iter()
    .filter(|(label, _)| !description.contains(&format!("{label}:")))
    .filter_map(|(label, key)| prop(r, key).map(|v| format!("**{label}:** {v}\n\n")))
    .chain(spell_hints(r, description))
    .collect();
    let (level_line, body) = match text.split_once("\n\n") {
        Some((first, rest))
            if (1..=6).contains(&first.split_whitespace().count())
                && ["level", "cantrip"]
                    .iter()
                    .any(|w| first.to_lowercase().contains(w)) =>
        {
            (format!("*{}*\n\n", first.trim()), rest)
        }
        _ => (String::new(), text.as_str()),
    };
    let text = format!("{level_line}{}{body}", missing.concat());
    ("spell", text)
}

const ABILITIES: [&str; 6] = [
    "strength",
    "dexterity",
    "constitution",
    "intelligence",
    "wisdom",
    "charisma",
];

fn spell_hints(r: &Value, description: &str) -> Vec<String> {
    let lower = description.to_lowercase();
    let mut out = Vec::new();
    if let Some(save) = prop(r, "Save")
        .map(|s| s.trim().to_lowercase())
        .filter(|s| ABILITIES.contains(&s.as_str()))
        && !lower.contains(&format!("{save} saving throw"))
        && !lower.contains(&format!("{save} save"))
    {
        out.push(format!("**Saving Throw:** {}\n\n", title(&save)));
    }
    if let Some(kind) = prop(r, "Damage Type").map(|k| k.trim().to_lowercase())
        && !kind.is_empty()
        && !lower.contains(&kind)
    {
        out.push(format!("**Damage Type:** {kind}\n\n"));
    }
    out
}

const SPELL_LABELS: [&str; 5] = ["Casting Time", "Range", "Components", "Duration", "Classes"];

fn spell_text(d: &str) -> String {
    let mut s = d.to_owned();
    for label in ["Casting Time:", "Range:", "Components:", "Classes:"] {
        if let Some(i) = s.find(&format!(" {label} ")) {
            s.replace_range(i..i + label.len() + 2, &format!("\n\n**{label}** "));
        }
    }
    if let Some(i) = s.find(" Duration: ") {
        let value_start = i + " Duration: ".len();
        let len = duration_len(&s[value_start..]);
        s.replace_range(i..value_start, "\n\n**Duration:** ");
        if let Some(n) = len {
            let end = i + "\n\n**Duration:** ".len() + n;
            if s[end..].starts_with(' ') {
                s.replace_range(end..end + 1, "\n\n");
            }
        }
    }
    for label in [
        "At Higher Levels.",
        "Using a Higher-Level Spell Slot.",
        "Cantrip Upgrade.",
    ] {
        if let Some(i) = s.find(&format!(" {label}")) {
            s.replace_range(i..i + label.len() + 1, &format!("\n\n**{label}**"));
        }
    }
    s
}

fn duration_len(s: &str) -> Option<usize> {
    for fixed in [
        "Instantaneous",
        "Until dispelled or triggered",
        "Until dispelled",
        "Permanent",
        "Special",
    ] {
        if s.starts_with(fixed) {
            return Some(fixed.len());
        }
    }
    let mut pos = 0;
    for prefix in ["Concentration, ", "up to "] {
        if s[pos..].starts_with(prefix) {
            pos += prefix.len();
        }
    }
    let digits = s[pos..].chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    pos += digits;
    let rest = s[pos..].strip_prefix(' ')?;
    for unit in [
        "rounds", "round", "minutes", "minute", "hours", "hour", "days", "day",
    ] {
        if rest.starts_with(unit) {
            return Some(pos + 1 + unit.len());
        }
    }
    None
}

fn monster(r: &Value, description: &str, e: &mut Map<String, Value>) -> (&'static str, String) {
    let valid = |c: &&str| {
        c.split_once('/').map_or_else(
            || c.parse::<f64>().is_ok_and(|v| (0.0..=30.0).contains(&v)),
            |(n, d)| n.parse::<u16>().is_ok() && matches!(d, "2" | "4" | "8"),
        )
    };
    let from_text = description
        .split("Challenge ")
        .skip(1)
        .filter_map(|rest| rest.split_whitespace().next())
        .find(valid);
    let cr = prop(r, "Challenge Rating")
        .filter(valid)
        .or(from_text)
        .unwrap_or("0");
    let size = prop(r, "Size")
        .and_then(|s| s.split_whitespace().next())
        .map(title)
        .unwrap_or_else(|| "Medium".into());
    let creature_type = prop(r, "Type")
        .map(title)
        .unwrap_or_else(|| "Creature".into());
    let mut header = format!("*{size} {creature_type}");
    if let Some(a) = prop(r, "Alignment") {
        header.push_str(", ");
        header.push_str(a);
    }
    header.push_str(&format!("*\n\n**CR** {cr}"));
    e.insert("cr".into(), json!(cr));
    e.insert("size".into(), json!(short(&size, 80)));
    e.insert("creature_type".into(), json!(short(&creature_type, 80)));
    (
        "monster",
        format!(
            "{header}\n\n{}",
            restore_labels(&restore_sections(description))
        ),
    )
}

fn item(r: &Value, description: &str, e: &mut Map<String, Value>) -> (&'static str, String) {
    let category = prop(r, "Item Type")
        .map(title)
        .unwrap_or_else(|| "Item".into());
    let rarity = prop(r, "Item Rarity")
        .map(str::to_lowercase)
        .filter(|x| RARITIES.contains(&x.as_str()))
        .map(|x| title(&x));
    let attunement = prop(r, "Requires Attunement").is_some_and(|a| {
        let a = a.to_lowercase();
        a.contains("attunement") || a == "yes"
    });
    e.insert("category".into(), json!(short(&category, 80)));
    let mut header = format!("*{category}");
    let kind = match rarity {
        Some(r) => {
            header.push_str(&format!(", {}", r.to_lowercase()));
            e.insert("rarity".into(), json!(r));
            if attunement {
                header.push_str(" (requires attunement)");
                e.insert("attunement".into(), json!(true));
            }
            "magic-item"
        }
        None => "equipment",
    };
    header.push('*');
    let name = r["name"].as_str().unwrap_or_default().trim();
    let body = strip_item_header(description, name);
    (
        kind,
        format!(
            "{header}\n\n{}",
            outside_tables(&dnd_tables::die_tables(&body), restore_labels)
        ),
    )
}

fn outside_tables(s: &str, f: fn(&str) -> String) -> String {
    s.split("\n\n")
        .map(|b| {
            if b.starts_with('|') {
                b.to_owned()
            } else {
                f(b)
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

const BACKGROUND_2024: [&str; 6] = [
    "Ability Scores",
    "Feat",
    "Skill Proficiencies",
    "Tool Proficiency",
    "Tool Proficiencies",
    "Equipment",
];

const BACKGROUND_2014: [&str; 4] = [
    "Skill Proficiencies:",
    "Tool Proficiencies:",
    "Languages:",
    "Equipment:",
];

fn species_text(d: &str) -> String {
    let mut s = d.to_owned();
    if let Some(p) = s.find(" Traits As ") {
        let q = s[..p].rfind(' ').map_or(0, |i| i + 1);
        let head = dnd_tables::trait_list(&s[..q], &["Creature Type", "Size", "Speed"], "", false);
        if head != s[..q] {
            s = format!(
                "{}\n\n### {}\n\n{}",
                head.trim_end(),
                &s[q..p + " Traits".len()],
                &s[p + " Traits ".len()..]
            );
        }
    }
    outside_tables(&dnd_tables::die_tables(&s), |b| {
        dnd_tables::sub_labels(&restore_labels(b))
    })
}

fn background_text(d: &str) -> String {
    let mut s = d.to_owned();
    for label in [
        "Skill Proficiencies",
        "Tool Proficiencies",
        "Languages",
        "Equipment",
        "Feature",
    ] {
        s = s.replace(&format!("{label} :"), &format!("{label}:"));
    }
    s = if s.starts_with("Ability Scores ") {
        dnd_tables::trait_list(&s, &BACKGROUND_2024, " GP ", true)
    } else {
        dnd_tables::trait_list(&s, &BACKGROUND_2014, " Feature:", false)
    };
    if let Some(i) = s.find("Feature: ") {
        let body = &s[i + "Feature: ".len()..];
        let words: Vec<&str> = body.split(' ').collect();
        let n = heading_len(&words);
        if n > 0 {
            let consumed: usize = words[..n].iter().map(|w| w.len() + 1).sum();
            let rest = body
                .get(consumed.min(body.len())..)
                .unwrap_or_default()
                .to_owned();
            let heading = words[..n].join(" ");
            s = format!("{}\n\n### Feature: {heading}\n\n{rest}", s[..i].trim_end());
        }
    }
    outside_tables(&dnd_tables::die_tables(&s), restore_labels)
}

fn restore_sections(d: &str) -> String {
    let mut s = d.to_owned();
    for h in SECTIONS {
        if let Some(rest) = s.strip_prefix(&format!("{h} ")) {
            s = format!("## {h}\n\n{rest}");
        }
        let pat = format!(". {h} ");
        let mut from = 0;
        while let Some(i) = s[from..].find(&pat).map(|i| i + from) {
            let after = i + pat.len();
            if s[after..].starts_with(|c: char| c.is_uppercase()) {
                let replacement = format!(".\n\n## {h}\n\n");
                s.replace_range(i..after, &replacement);
                from = i + replacement.len();
            } else {
                from = after;
            }
        }
    }
    s
}

const BOILERPLATE: &str = "These D&D 5E Free Basic Rules only contain a fraction";

const INLINE_LABELS: [&str; 12] = [
    "Hit",
    "Miss",
    "Hit or Miss",
    "Failure",
    "Success",
    "Trigger",
    "Response",
    "Melee Weapon Attack",
    "Ranged Weapon Attack",
    "Melee Spell Attack",
    "Ranged Spell Attack",
    "Melee or Ranged Weapon Attack",
];

pub const SENTENCE_STARTS: [&str; 30] = [
    "You",
    "Your",
    "When",
    "As",
    "If",
    "The",
    "At",
    "Starting",
    "Beginning",
    "Whenever",
    "While",
    "In",
    "Once",
    "Choose",
    "This",
    "These",
    "A",
    "An",
    "Each",
    "Some",
    "Many",
    "Until",
    "For",
    "On",
    "By",
    "After",
    "Before",
    "Whether",
    "Although",
    "Most",
];

const CONNECTORS: [&str; 11] = [
    "of", "the", "and", "a", "an", "to", "in", "on", "with", "for", "or",
];

fn clean_description(d: &str) -> String {
    let mut s: String = d
        .chars()
        .filter(|c| !matches!(c, '\u{200B}'..='\u{200D}' | '\u{2060}' | '\u{FEFF}'))
        .collect();
    if let Some(i) = s.find(BOILERPLATE)
        && let Some(j) = s[i..].find("to fight!")
    {
        s.replace_range(i..i + j + "to fight!".len(), "");
    }
    s.replace(" , ", ", ").replace(" ; ", "; ")
}

fn is_heading_word(w: &str, first: bool) -> bool {
    let starts_upper = w.starts_with(|c: char| c.is_uppercase());
    if first {
        return starts_upper && !SENTENCE_STARTS.contains(&w);
    }
    (starts_upper && !SENTENCE_STARTS.contains(&w)) || CONNECTORS.contains(&w)
}

fn heading_len(words: &[&str]) -> usize {
    let mut n = 0;
    while n < words.len() && n < 7 && is_heading_word(words[n], n == 0) {
        n += 1;
    }
    while n > 0 && CONNECTORS.contains(&words[n - 1]) {
        n -= 1;
    }
    n
}

fn restore_features(d: &str, known: &[String]) -> String {
    let mut out = String::with_capacity(d.len() + 512);
    let mut rest = d;
    while let Some(i) = rest.find("Level ") {
        let after = &rest[i + 6..];
        let digits = after.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 || !after[digits..].starts_with(": ") {
            out.push_str(&rest[..i + 6]);
            rest = after;
            continue;
        }
        let body = &after[digits + 2..];
        let words: Vec<&str> = body.split(' ').collect();
        let n = known
            .iter()
            .map(|k| k.split(' ').collect::<Vec<_>>())
            .filter(|k| words.starts_with(k))
            .map(|k| k.len())
            .max()
            .unwrap_or_else(|| heading_len(&words));
        if n == 0 {
            out.push_str(&rest[..i + 6]);
            rest = after;
            continue;
        }
        out.push_str(rest[..i].trim_end());
        let heading = words[..n].join(" ");
        out.push_str(&format!(
            "\n\n### Level {}: {heading}\n\n",
            &after[..digits]
        ));
        let consumed: usize = words[..n].iter().map(|w| w.len() + 1).sum();
        rest = body.get(consumed.min(body.len())..).unwrap_or_default();
    }
    out.push_str(rest);
    restore_level_phrases(&out)
}

fn restore_level_phrases(d: &str) -> String {
    const MARKERS: [&str; 6] = [
        "At ",
        "Beginning at ",
        "Starting at ",
        "When you reach ",
        "Starting when ",
        "Beginning when ",
    ];
    let mut cuts: Vec<(usize, usize)> = Vec::new();
    for marker in MARKERS {
        let mut from = 0;
        while let Some(i) = d[from..].find(marker).map(|i| i + from) {
            from = i + marker.len();
            let tail: String = d[from..].chars().take(40).collect();
            if !tail.contains(" level")
                || !tail.starts_with(|c: char| c.is_ascii_digit() || c == 'y')
            {
                continue;
            }
            let before = &d[..i];
            let boundary = [". ", "\n"]
                .iter()
                .filter_map(|p| before.rfind(p).map(|j| j + p.len()))
                .max()
                .unwrap_or(0);
            let words: Vec<&str> = before[boundary..].split_whitespace().collect();
            if words.is_empty() || heading_len(&words) != words.len() {
                continue;
            }
            cuts.push((boundary, i));
        }
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut out = String::with_capacity(d.len() + cuts.len() * 8);
    let mut last = 0;
    for (start, marker) in cuts {
        if start < last {
            continue;
        }
        out.push_str(d[last..start].trim_end());
        out.push_str(&format!("\n\n### {}\n\n", d[start..marker].trim()));
        last = marker;
    }
    out.push_str(&d[last..]);
    out
}

fn restore_labels(d: &str) -> String {
    let mut out = String::with_capacity(d.len() + 256);
    let mut rest = d;
    loop {
        let dot = rest.find(" . ");
        let colon = rest.find(" : ");
        let (i, sep) = match (dot, colon) {
            (Some(a), Some(b)) if b < a => (b, ':'),
            (Some(a), _) => (a, '.'),
            (None, Some(b)) => (b, ':'),
            (None, None) => break,
        };
        let (before, after) = (&rest[..i], &rest[i + 3..]);
        let boundary = [". ", "! ", "? ", "\n"]
            .iter()
            .filter_map(|p| before.rfind(p).map(|j| j + p.len()))
            .max()
            .unwrap_or(0);
        let label = before[boundary..].trim();
        let is_label = !label.is_empty()
            && label.len() <= 70
            && label.split_whitespace().count() <= 8
            && label.starts_with(|c: char| c.is_uppercase());
        if is_label && INLINE_LABELS.contains(&label) {
            out.push_str(&before[..boundary]);
            out.push_str(&format!("*{label}:* "));
        } else if is_label {
            out.push_str(before[..boundary].trim_end());
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&format!("**{label}.** "));
        } else {
            out.push_str(before);
            out.push(sep);
            out.push(' ');
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

const FREE_RULES_2014: &str = "Free Basic Rules (2014)";

const FREE_SUBCLASSES: [(&str, &str); 12] = [
    ("Barbarian", "Path of the Berserker"),
    ("Bard", "College of Lore"),
    ("Cleric", "Life Domain"),
    ("Druid", "Circle of the Land"),
    ("Fighter", "Champion"),
    ("Monk", "Way of the Open Hand"),
    ("Paladin", "Oath of Devotion"),
    ("Ranger", "Hunter"),
    ("Rogue", "Thief"),
    ("Sorcerer", "Draconic Bloodline"),
    ("Warlock", "The Fiend"),
    ("Wizard", "School of Evocation"),
];

const SUBCLASS_BOOKS: [&str; 14] = [
    "Xanathar's Guide to Everything",
    "Sword Coast Adventurer's Guide",
    "Tasha's Cauldron of Everything",
    "Explorer's Guide to Wildemount",
    "Mythic Odysseys of Theros",
    "Van Richten's Guide to Ravenloft",
    "Fizban's Treasury of Dragons",
    "Strixhaven: A Curriculum of Chaos",
    "Guildmasters' Guide to Ravnica",
    "Dungeon Master's Guide",
    "The Book of Many Things",
    "Bigby Presents: Glory of the Giants",
    "Acquisitions Incorporated",
    "Player's Handbook",
];

const NAMES_ONLY: &str = "*Only the names are in this file; their rules aren't included.*";

fn class_text(raw: &str) -> String {
    let (gridded, features) = dnd_tables::level_grids(&dnd_tables::drop_cards(raw));
    let structured =
        dnd_tables::class_rules(&dnd_tables::core_traits(&dnd_tables::slot_grid(&gridded)));
    let d = structured.as_str();
    let mut tail_from = d.len() * 4 / 5;
    while !d.is_char_boundary(tail_from) {
        tail_from -= 1;
    }
    let text = match d[tail_from..].rfind(" Subclasses ").map(|i| i + tail_from) {
        Some(i) if d.is_char_boundary(i) => {
            let mut names = d[i + " Subclasses ".len()..].to_owned();
            for book in SUBCLASS_BOOKS {
                names = names.replace(&format!(" {book} "), &format!("\n\n**{book}:** "));
            }
            format!("{}\n\n### Subclasses\n\n{NAMES_ONLY}\n\n{names}", &d[..i])
        }
        _ => d.to_owned(),
    };
    let names = dnd_tables::feature_names(&features);
    let headed = dnd_tables::feature_headings(&restore_features(&text, &names), &names);
    outside_tables(&dnd_tables::die_tables(&headed), |b| {
        dnd_tables::sub_labels(&restore_labels(b))
    })
}

fn subclass_start(d: &str, name: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(i) = d[from..].find(name).map(|i| i + from) {
        from = i + name.len();
        let before = d[..i].to_lowercase();
        let after = &d[from..];
        if i * 5 > d.len() * 2
            && !before.ends_with("the ")
            && !before.ends_with("a ")
            && !before.ends_with("of ")
            && after.starts_with(' ')
            && after[1..].starts_with(|c: char| c.is_uppercase())
        {
            return Some(i);
        }
    }
    None
}

fn split_free_subclass(mut class: Value) -> Vec<Value> {
    let (Some(name), Some(book)) = (
        class["name"].as_str().map(str::to_owned),
        class["book"].as_str().map(str::to_owned),
    ) else {
        return vec![class];
    };
    let Some((_, sub)) = FREE_SUBCLASSES.iter().find(|(c, _)| *c == name) else {
        return vec![class];
    };
    if book != FREE_RULES_2014 {
        return vec![class];
    }
    let raw = class["source_text"].as_str().unwrap_or_default().to_owned();
    let Some(start) = subclass_start(&raw, sub) else {
        return vec![class];
    };
    let head = raw[..start].trim_end();
    let split = head.rfind(". ").map_or(0, |j| j + 2);
    let words: Vec<&str> = head[split..].split_whitespace().collect();
    let class_part = if !words.is_empty() && heading_len(&words) == words.len() {
        format!(
            "{}\n\n### {}\n\n**Subclass in this book:** {sub} (a separate entry).",
            class_text(head[..split].trim_end()),
            words.join(" ")
        )
    } else {
        format!(
            "{}\n\n**Subclass in this book:** {sub} (a separate entry).",
            class_text(head)
        )
    };
    let sub_part = restore_labels(&restore_features(raw[start + sub.len()..].trim(), &[]));
    class["text"] = json!(truncate_bytes(&class_part, 64 * 1024));
    let subclass = json!({
        "kind": "subclass",
        "name": sub,
        "class": name,
        "book": book,
        "rules": "2014",
        "text": truncate_bytes(&sub_part, 64 * 1024),
    });
    vec![class, subclass]
}

fn strip_item_header(d: &str, name: &str) -> String {
    let Some(rest) = d.strip_prefix(name).map(str::trim_start) else {
        return d.to_owned();
    };
    let window: String = rest.chars().take(140).collect::<String>().to_lowercase();
    let mut cut = None;
    for r in RARITIES {
        if let Some(i) = window.find(r) {
            let end = i + r.len();
            let end = if window[end..].starts_with(" (requires attunement") {
                window[end..].find(')').map_or(end, |j| end + j + 1)
            } else {
                end
            };
            cut = Some(cut.map_or(end, |c: usize| c.max(end)));
        }
    }
    match cut {
        Some(c) if rest.is_char_boundary(c) => rest[c..].trim_start().to_owned(),
        _ => d.to_owned(),
    }
}

pub struct Draft {
    pub name: NoteTitle,
    pub facets: Facets,
    pub text: NoteBody,
    pub book: Option<NoteTitle>,
    pub rules: RulesVersion,
}

pub fn to_draft(v: &Value) -> Option<Draft> {
    let str_of = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
    let kind = Kind::parse(v.get("kind")?.as_str()?)?;
    let (mut facets, _) = default_facets(kind);
    match &mut facets {
        Facets::Spell {
            level,
            school,
            classes,
            ritual,
            concentration,
        } => {
            *level = v.get("level").and_then(Value::as_u64).unwrap_or(0).min(9) as u8;
            if let Some(s) = str_of("school") {
                *school = s;
            }
            *classes = v
                .get("classes")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|c| c.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            *ritual = v.get("ritual").and_then(Value::as_bool).unwrap_or(false);
            *concentration = v
                .get("concentration")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        }
        Facets::Monster {
            cr,
            creature_type,
            size,
        } => {
            if let Some(c) = str_of("cr").as_deref().and_then(cr_of) {
                *cr = c;
            }
            if let Some(t) = str_of("creature_type") {
                *creature_type = t;
            }
            if let Some(s) = str_of("size") {
                *size = s;
            }
        }
        Facets::MagicItem {
            rarity,
            category,
            attunement,
        } => {
            if let Some(r) = str_of("rarity") {
                *rarity = r;
            }
            if let Some(c) = str_of("category") {
                *category = c;
            }
            *attunement = v
                .get("attunement")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        }
        Facets::Equipment { category } | Facets::Feat { category } => {
            if let Some(c) = str_of("category") {
                *category = c;
            }
        }
        Facets::Subclass { class } | Facets::Feature { class, .. } => {
            if let Some(c) = str_of("class") {
                *class = c;
            }
        }
        Facets::Class
        | Facets::Species
        | Facets::Background
        | Facets::Trait
        | Facets::Condition => {}
    }
    let rules = match str_of("rules").as_deref() {
        Some("2024") => RulesVersion::R2024,
        Some("level-up") => RulesVersion::LevelUp,
        _ => RulesVersion::R2014,
    };
    Some(Draft {
        name: NoteTitle::new(&str_of("name")?).ok()?,
        facets,
        text: NoteBody::new(str_of("text").unwrap_or_default()).ok()?,
        book: str_of("book").and_then(|b| NoteTitle::new(&b).ok()),
        rules,
    })
}

fn cr_of(s: &str) -> Option<ChallengeRating> {
    parse_cr(s).or_else(|| {
        let e = (s.parse::<f64>().ok()? * 8.0).round();
        (0.0..=320.0)
            .contains(&e)
            .then(|| ChallengeRating::from_eighths(e as u16))
    })
}

struct Records<F>(F);

impl<'de, F: FnMut(Value)> serde::de::Visitor<'de> for Records<F> {
    type Value = ();

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a JSON array of records")
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(mut self, mut seq: A) -> Result<(), A::Error> {
        while let Some(v) = seq.next_element::<Value>()? {
            (self.0)(v);
        }
        Ok(())
    }
}

pub fn each_record(
    reader: impl std::io::Read,
    f: impl FnMut(Value),
) -> Result<(), serde_json::Error> {
    use serde::Deserializer;
    serde_json::Deserializer::from_reader(reader).deserialize_seq(Records(f))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spell_properties_fill_what_the_text_lacks() {
        let raw = json!({"name": "Arrhythmia", "description": "Your target's heart skips. It takes 2d8 damage on a failure.", "book": "Deep Magic",
            "properties": {"Category": "Spells", "Level": 2, "School": "necromancy", "Save": "Constitution", "Damage Type": "necrotic", "Casting Time": "1 action"}});
        let text = convert("spells", &raw)[0]["text"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(text.contains("**Saving Throw:** Constitution"), "{text}");
        assert!(text.contains("**Damage Type:** necrotic"), "{text}");
        let named = json!({"name": "Zap", "description": "Make a Dexterity saving throw or take 1d6 fire damage.", "book": "X",
            "properties": {"Category": "Spells", "Level": 1, "School": "evocation", "Save": "Dexterity", "Damage Type": "fire"}});
        let text = convert("spells", &named)[0]["text"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(
            !text.contains("Saving Throw:") && !text.contains("Damage Type:"),
            "{text}"
        );
    }

    #[test]
    fn streamed_records_become_drafts() {
        let raw = r#"[{"name":"Goblin","description":"Traits Nimble Escape : Hides.","book":"Monster Manual","publisher":"WotC","properties":{"Category":"Monsters","Size":"Small","Type":"humanoid","Challenge Rating":0.25}},
            {"name":"Fireball","description":"3rd-Level Evocation Casting Time : 1 action","book":"Player's Handbook (2024)","properties":{"Category":"Spells","Level":3,"School":"evocation"}}]"#;
        let mut drafts = Vec::new();
        each_record(raw.as_bytes(), |mut v| {
            numbers_as_text(&mut v);
            let file = match v["properties"]["Category"].as_str() {
                Some("Spells") => "spells",
                _ => "monsters",
            };
            drafts.extend(convert(file, &v).iter().filter_map(to_draft));
        })
        .unwrap();
        assert_eq!(drafts.len(), 2);
        assert_eq!(drafts[0].name.as_str(), "Goblin");
        assert!(matches!(
            drafts[0].facets,
            Facets::Monster { cr, ref size, .. } if cr.eighths() == 2 && size == "Small"
        ));
        assert_eq!(drafts[0].rules, RulesVersion::R2014);
        assert!(
            matches!(drafts[1].facets, Facets::Spell { level: 3, ref school, .. } if school == "Evocation")
        );
        assert_eq!(drafts[1].rules, RulesVersion::R2024);
        assert!(each_record(&b"{}"[..], |_| {}).is_err());
    }

    #[test]
    fn monster_text_gets_its_structure_back() {
        let d = "A guardian. Traits Dive Attack . If it dives, extra damage. Actions Multiattack . Two attacks. Talon . Melee Weapon Attack : +6 to hit.";
        let t = restore_labels(&restore_sections(d));
        assert!(
            t.contains("## Traits\n\n**Dive Attack.** If it dives"),
            "{t}"
        );
        assert!(
            t.contains("## Actions\n\n**Multiattack.** Two attacks."),
            "{t}"
        );
        assert!(t.contains("**Talon.** *Melee Weapon Attack:* +6"), "{t}");
    }

    #[test]
    fn spell_headers_split_into_lines() {
        let d = "Level 1 Abjuration (Ranger, Wizard) Casting Time: 1 minute or Ritual Range: 30 feet Components: V, S Duration: 8 hours You set an alarm. Using a Higher-Level Spell Slot. More.";
        let t = spell_text(d);
        assert!(t.contains("\n\n**Range:** 30 feet"), "{t}");
        assert!(t.contains("**Duration:** 8 hours\n\nYou set"), "{t}");
        assert!(
            t.contains("\n\n**Using a Higher-Level Spell Slot.** More."),
            "{t}"
        );
        let mut e = Map::new();
        let r = json!({ "properties": { "School": "abjuration", "Casting Time": "1 minute or Ritual" } });
        spell(&r, d, &mut e);
        assert_eq!(e["level"], json!(1));
        assert_eq!(e["school"], json!("Abjuration"));
        assert_eq!(e["classes"], json!(["Ranger", "Wizard"]));
        assert_eq!(e["ritual"], json!(true));
    }

    #[test]
    fn boilerplate_and_invisible_characters_go() {
        let d = "These D&D 5E Free Basic Rules only contain a fraction of the races ... to fight! Elf Traits charges\u{2060} toward";
        assert_eq!(clean_description(d), " Elf Traits charges toward");
        assert_eq!(
            clean_description("Short Rest , and Bludgeoning , Piercing"),
            "Short Rest, and Bludgeoning, Piercing"
        );
    }

    #[test]
    fn colon_labels_and_inline_attack_labels() {
        let t = restore_labels(&restore_sections(
            "Traits Nimble Escape : Disengage as a bonus action. Scimitar : Melee Weapon Attack: +4 to hit. Hit : 5 (1d6 + 2) damage.",
        ));
        assert!(
            t.contains("## Traits\n\n**Nimble Escape.** Disengage"),
            "{t}"
        );
        assert!(t.contains("\n\n**Scimitar.** Melee"), "{t}");
        assert!(t.contains("*Hit:* 5 (1d6 + 2)"), "{t}");
        let t = restore_labels("A guardian. Talon . Melee Weapon Attack : +6 to hit.");
        assert!(
            t.contains("\n\n**Talon.** *Melee Weapon Attack:* +6 to hit."),
            "{t}"
        );
    }

    #[test]
    fn class_features_become_headings() {
        let t = restore_features(
            "Intro. Level 1: Rage You can imbue yourself. Level 3: Barbarian Subclass You gain a subclass.",
            &[],
        );
        assert!(t.contains("\n\n### Level 1: Rage\n\nYou can imbue"), "{t}");
        assert!(
            t.contains("### Level 3: Barbarian Subclass\n\nYou gain"),
            "{t}"
        );
        let t = restore_features(
            "You can rage. Persistent Rage Beginning at 15th level, your rage is fierce. Primal Champion At 20th level, you embody power.",
            &[],
        );
        assert!(
            t.contains("\n\n### Persistent Rage\n\nBeginning at 15th level"),
            "{t}"
        );
        assert!(
            t.contains("\n\n### Primal Champion\n\nAt 20th level"),
            "{t}"
        );
    }

    #[test]
    fn species_traits_2024() {
        let t = species_text(
            "Elves are old. Creature Type Humanoid Size Medium (about 5–6 feet tall) Speed 30 feet Elf Traits As an Elf, you have these traits. Darkvision. You have Darkvision. Trance. You don’t sleep.",
        );
        assert_eq!(
            t,
            "Elves are old.\n\n- **Creature Type:** Humanoid\n- **Size:** Medium (about 5–6 feet tall)\n- **Speed:** 30 feet\n\n### Elf Traits\n\nAs an Elf, you have these traits.\n\n**Darkvision.** You have Darkvision.\n\n**Trance.** You don’t sleep."
        );
    }

    #[test]
    fn free_rules_subclasses_are_split_out() {
        let raw = &format!(
            "{}Rage In battle you fight. Primal Paths Path of the Berserker For some barbarians, rage is a means to an end. Frenzy Starting when you choose this path at 3rd level, you can go into a frenzy.",
            "Barbarians are fierce warriors of the wild lands. ".repeat(6)
        );
        let class = json!({ "kind": "class", "name": "Barbarian", "book": FREE_RULES_2014, "text": "", "source_text": raw });
        let out = split_free_subclass(class);
        assert_eq!(out.len(), 2);
        assert!(
            out[0]["text"]
                .as_str()
                .unwrap()
                .contains("**Subclass in this book:** Path of the Berserker")
        );
        assert!(!out[0]["text"].as_str().unwrap().contains("means to an end"));
        assert_eq!(out[1]["kind"], json!("subclass"));
        assert_eq!(out[1]["class"], json!("Barbarian"));
        assert!(
            out[1]["text"]
                .as_str()
                .unwrap()
                .starts_with("For some barbarians"),
            "{}",
            out[1]["text"]
        );
        let other = json!({ "kind": "class", "name": "Barbarian", "book": "Player's Handbook", "text": "", "source_text": raw });
        assert_eq!(
            split_free_subclass(other).len(),
            1,
            "only the Free Basic Rules embed a subclass"
        );
        let names = class_text(&format!(
            "{} Subclasses Path of the Berserker Path of the Totem Warrior",
            "x ".repeat(200)
        ));
        assert!(
            names.contains("### Subclasses\n\n*Only the names"),
            "{names}"
        );
    }

    #[test]
    fn item_headers_are_not_repeated() {
        let d = "Bag of Holding Wondrous item, uncommon This bag has an interior space.";
        assert_eq!(
            strip_item_header(d, "Bag of Holding"),
            "This bag has an interior space."
        );
        let d = "Flame Tongue Weapon (any sword), rare (requires attunement) You can use a bonus action.";
        assert_eq!(
            strip_item_header(d, "Flame Tongue"),
            "You can use a bonus action."
        );
        assert_eq!(
            strip_item_header("Rope, hempen. 50 feet.", "Torch"),
            "Rope, hempen. 50 feet."
        );
    }

    #[test]
    fn items_without_a_real_rarity_are_equipment() {
        let mut e = Map::new();
        let r =
            json!({ "properties": { "Item Type": "Adventuring Gear", "Item Rarity": "Standard" } });
        assert_eq!(item(&r, "A rope.", &mut e).0, "equipment");
        let mut e = Map::new();
        let r = json!({ "properties": { "Item Type": "Wondrous item", "Item Rarity": "very rare", "Requires Attunement": "requires attunement" } });
        assert_eq!(item(&r, "Shiny.", &mut e).0, "magic-item");
        assert_eq!(e["rarity"], json!("Very Rare"));
        assert_eq!(e["attunement"], json!(true));
        assert_eq!(rules_of("Player's Handbook (2024)"), "2024");
    }
}
