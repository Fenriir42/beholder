use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};

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

pub fn convert_all(dir: &Path) -> Result<Vec<Converted>> {
    let mut out = Vec::new();
    for name in FILES {
        let path = dir.join(format!("{name}.json"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut rows: Vec<Value> =
            serde_json::from_str(&text).with_context(|| path.display().to_string())?;
        rows.iter_mut().for_each(numbers_as_text);
        let entries: Vec<Value> = rows.iter().filter_map(|r| convert(name, r)).collect();
        let n = entries.len();
        let file = json!({ "format": "beholder-library", "version": 1, "entries": entries });
        out.push(Converted {
            file: format!("{name}.json"),
            entries: n,
            json: serde_json::to_string_pretty(&file)?,
        });
    }
    Ok(out)
}

fn numbers_as_text(r: &mut Value) {
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

fn convert(file: &str, r: &Value) -> Option<Value> {
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
        "classes" => ("class", restore_labels(&restore_features(description))),
        "species" => ("species", restore_labels(description)),
        _ => ("background", restore_labels(description)),
    };
    e.insert("kind".into(), json!(kind));
    e.insert("text".into(), json!(truncate_bytes(&text, 64 * 1024)));
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

fn spell(r: &Value, description: &str, e: &mut Map<String, Value>) -> (&'static str, String) {
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
    ("spell", spell_text(description))
}

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
    (kind, format!("{header}\n\n{}", restore_labels(&body)))
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

const SENTENCE_STARTS: [&str; 30] = [
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
    {        s.replace_range(i..i + j + "to fight!".len(), "");
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

fn restore_features(d: &str) -> String {
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
        let n = heading_len(&words);
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
            from = i + marker.len();            let tail: String = d[from..].chars().take(40).collect();
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let d = "These D&D 5E Free Basic Rules only contain a fraction of the races ... to fight! Elf Traits charges\u{2060} toward";        assert_eq!(clean_description(d), " Elf Traits charges toward");
        assert_eq!(clean_description("Short Rest , and Bludgeoning , Piercing"), "Short Rest, and Bludgeoning, Piercing");
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
        );
        assert!(t.contains("\n\n### Level 1: Rage\n\nYou can imbue"), "{t}");
        assert!(
            t.contains("### Level 3: Barbarian Subclass\n\nYou gain"),
            "{t}"
        );
        let t = restore_features(
            "You can rage. Persistent Rage Beginning at 15th level, your rage is fierce. Primal Champion At 20th level, you embody power.",
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
