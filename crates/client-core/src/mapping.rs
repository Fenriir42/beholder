use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;

use beholder_core::compendium::{ChallengeRating, Facets, Kind, RulesVersion};
pub use beholder_core::mapping::{Field, Profile};
use beholder_core::note::MAX_BODY_BYTES;
use beholder_core::paste::{default_facets, parse_cr};
use beholder_core::{NoteBody, NoteTitle};
use serde::de::{self, DeserializeSeed, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::Value;

use crate::library_file::Parsed;

const STOP: &str = "beholder: enough";

pub const SAMPLE: usize = 20;

fn names(f: Field) -> &'static [&'static str] {
    match f {
        Field::Level => &["level", "spell level"],
        Field::School => &["school", "magic school"],
        Field::Classes => &["classes", "class list", "spell lists"],
        Field::Ritual => &["ritual"],
        Field::Concentration => &["concentration"],
        Field::Cr => &["cr", "challenge rating", "challenge"],
        Field::CreatureType => &["creature type", "monster type"],
        Field::Size => &["size"],
        Field::Rarity => &["rarity", "item rarity"],
        Field::Category => &["category", "item type", "item category"],
        Field::Attunement => &["attunement", "requires attunement"],
        Field::Class => &["class", "parent class", "base class"],
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Probe {
    pub entries: Vec<String>,
    pub sample: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mapped {
    pub parsed: Parsed,
    pub partial: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum MappingError {
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no list of entries found in this file")]
    NoEntries,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

fn stopped(e: &serde_json::Error) -> bool {
    e.is_data() && e.to_string().starts_with(STOP)
}

struct Finder<'a, F> {
    path: &'a [String],
    each: &'a mut F,
}

impl<'de, F: FnMut(Value) -> bool> DeserializeSeed<'de> for Finder<'_, F> {
    type Value = bool;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<bool, D::Error> {
        d.deserialize_any(self)
    }
}

macro_rules! skip_scalars {
    ($v:ty) => {
        fn visit_bool<E: de::Error>(self, _: bool) -> Result<$v, E> {
            Ok(Default::default())
        }
        fn visit_i64<E: de::Error>(self, _: i64) -> Result<$v, E> {
            Ok(Default::default())
        }
        fn visit_u64<E: de::Error>(self, _: u64) -> Result<$v, E> {
            Ok(Default::default())
        }
        fn visit_f64<E: de::Error>(self, _: f64) -> Result<$v, E> {
            Ok(Default::default())
        }
        fn visit_str<E: de::Error>(self, _: &str) -> Result<$v, E> {
            Ok(Default::default())
        }
        fn visit_unit<E: de::Error>(self) -> Result<$v, E> {
            Ok(Default::default())
        }
    };
}

impl<'de, F: FnMut(Value) -> bool> Visitor<'de> for Finder<'_, F> {
    type Value = bool;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON array or object")
    }

    skip_scalars!(bool);

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<bool, A::Error> {
        if !self.path.is_empty() {
            while seq.next_element::<IgnoredAny>()?.is_some() {}
            return Ok(false);
        }
        while let Some(v) = seq.next_element::<Value>()? {
            if !(self.each)(v) {
                return Err(de::Error::custom(STOP));
            }
        }
        Ok(true)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<bool, A::Error> {
        let Finder { path, each } = self;
        let mut found = false;
        while let Some(k) = map.next_key::<String>()? {
            if !found && path.first() == Some(&k) {
                found = map.next_value_seed(Finder {
                    path: &path[1..],
                    each: &mut *each,
                })?;
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(found)
    }
}

pub fn each(
    reader: impl Read,
    path: &[String],
    mut f: impl FnMut(Value) -> bool,
) -> Result<bool, MappingError> {
    let mut de = serde_json::Deserializer::from_reader(reader);
    match (Finder { path, each: &mut f }).deserialize(&mut de) {
        Ok(found) => Ok(found),
        Err(e) if stopped(&e) => Ok(true),
        Err(e) => Err(e.into()),
    }
}

struct Prober<'a> {
    path: Vec<String>,
    out: &'a mut Option<Probe>,
}

impl<'de> DeserializeSeed<'de> for Prober<'_> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Prober<'_> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JSON")
    }

    skip_scalars!(());

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        let Some(first) = seq.next_element::<Value>()? else {
            return Ok(());
        };
        if !first.is_object() || self.out.is_some() {
            while seq.next_element::<IgnoredAny>()?.is_some() {}
            return Ok(());
        }
        let mut sample = vec![first];
        while sample.len() < SAMPLE {
            match seq.next_element::<Value>()? {
                Some(v) => sample.push(v),
                None => break,
            }
        }
        *self.out = Some(Probe {
            entries: self.path,
            sample,
        });
        Err(de::Error::custom(STOP))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let Prober { path, out } = self;
        while let Some(k) = map.next_key::<String>()? {
            if out.is_none() && path.len() < 3 {
                let mut next = path.clone();
                next.push(k);
                map.next_value_seed(Prober {
                    path: next,
                    out: &mut *out,
                })?;
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(())
    }
}

pub fn probe(reader: impl Read) -> Result<Probe, MappingError> {
    let mut de = serde_json::Deserializer::from_reader(reader);
    let mut out = None;
    match (Prober {
        path: Vec::new(),
        out: &mut out,
    })
    .deserialize(&mut de)
    {
        Ok(()) => {}
        Err(e) if stopped(&e) => {}
        Err(e) => return Err(e.into()),
    }
    out.ok_or(MappingError::NoEntries)
}

pub fn get<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(v, |cur, key| cur.as_object()?.get(key))
}

fn text_of(v: &Value) -> Option<String> {
    let s = match v {
        Value::String(s) => s.trim().to_owned(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(items) => items
            .iter()
            .filter_map(text_of)
            .collect::<Vec<_>>()
            .join(", "),
        Value::Null | Value::Object(_) => return None,
    };
    (!s.is_empty()).then_some(s)
}

pub fn leaf_keys(sample: &[Value]) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    let mut add = |k: String| {
        if !keys.contains(&k) {
            keys.push(k);
        }
    };
    for v in sample {
        let Some(obj) = v.as_object() else { continue };
        for (k, val) in obj {
            match val.as_object() {
                Some(inner) => inner.keys().for_each(|ik| add(format!("{k}.{ik}"))),
                None => add(k.clone()),
            }
        }
    }
    keys
}

fn norm(key: &str) -> String {
    key.rsplit('.')
        .next()
        .unwrap_or(key)
        .to_lowercase()
        .replace(['_', '-'], " ")
}

fn find_key(keys: &[String], names: &[&str], taken: &[&str]) -> Option<String> {
    names.iter().find_map(|n| {
        keys.iter()
            .find(|k| norm(k) == *n && !taken.contains(&k.as_str()))
            .cloned()
    })
}

pub fn guess_kind(value: &str) -> Option<Kind> {
    let v = value.trim().to_lowercase().replace(['_', ' '], "-");
    let single = v
        .strip_suffix("es")
        .filter(|s| s.ends_with("ss"))
        .or_else(|| v.strip_suffix('s'));
    let candidates = [Some(v.as_str()), single];
    for c in candidates.into_iter().flatten() {
        if let Some(k) = Kind::parse(c) {
            return Some(k);
        }
        let k = match c {
            "creature" | "npc" | "beast" => Kind::Monster,
            "item" | "magic-item" | "wondrous-item" => Kind::MagicItem,
            "weapon" | "armor" | "gear" | "tool" => Kind::Equipment,
            "race" | "lineage" => Kind::Species,
            "ability" | "class-feature" => Kind::Feature,
            "archetype" => Kind::Subclass,
            _ => continue,
        };
        return Some(k);
    }
    None
}

pub fn suggest(probe: &Probe) -> Profile {
    let keys = leaf_keys(&probe.sample);
    let name_key = find_key(&keys, &["name", "title", "label"], &[])
        .or_else(|| {
            keys.iter()
                .find(|k| {
                    probe
                        .sample
                        .iter()
                        .any(|v| get(v, k).is_some_and(Value::is_string))
                })
                .cloned()
        })
        .unwrap_or_else(|| "name".into());
    let text_keys: Vec<String> = find_key(
        &keys,
        &["description", "desc", "text", "body", "content", "entries"],
        &[],
    )
    .into_iter()
    .collect();
    let kind_key = ["kind", "type", "category"]
        .iter()
        .flat_map(|n| keys.iter().filter(move |k| norm(k) == *n))
        .max_by_key(|k| {
            probe
                .sample
                .iter()
                .filter(|v| {
                    get(v, k)
                        .and_then(text_of)
                        .as_deref()
                        .and_then(guess_kind)
                        .is_some()
                })
                .count()
        })
        .filter(|k| {
            probe.sample.iter().any(|v| {
                get(v, k)
                    .and_then(text_of)
                    .as_deref()
                    .and_then(guess_kind)
                    .is_some()
            })
        })
        .cloned();
    let mut kinds = BTreeMap::new();
    if let Some(k) = &kind_key {
        for v in &probe.sample {
            if let Some(value) = get(v, k).and_then(text_of)
                && let Some(kind) = guess_kind(&value)
            {
                kinds.insert(value.to_lowercase(), kind);
            }
        }
    }
    let book_key = find_key(&keys, &["book", "source", "document", "sourcebook"], &[]);
    let mut taken: Vec<&str> = vec![name_key.as_str()];
    taken.extend(text_keys.iter().map(String::as_str));
    taken.extend(kind_key.as_deref());
    taken.extend(book_key.as_deref());
    let mut fields = BTreeMap::new();
    for f in Field::ALL {
        if let Some(k) = find_key(&keys, names(f), &taken) {
            fields.insert(f, k);
        }
    }
    if !fields.contains_key(&Field::CreatureType)
        && kinds.values().any(|k| *k == Kind::Monster)
        && let Some(k) = find_key(&keys, &["type"], &taken)
    {
        fields.insert(Field::CreatureType, k);
    }
    Profile {
        name: String::new(),
        entries: probe.entries.clone(),
        name_key,
        text_keys,
        kind_key,
        kinds,
        default_kind: Kind::Trait,
        rules_from_book: book_key.is_some(),
        book_key,
        book: None,
        rules: RulesVersion::R2014,
        fields,
    }
}

fn truncate(s: &str, max: usize) -> String {
    let mut end = s.len().min(max);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_owned()
}

fn short(s: &str) -> String {
    s.chars().take(80).collect()
}

fn level_of(s: &str) -> Option<u8> {
    if s.to_lowercase().contains("cantrip") {
        return Some(0);
    }
    let digits: String = s
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

fn cr_of(s: &str) -> Option<ChallengeRating> {
    let token = s.split_whitespace().next()?;
    parse_cr(token).or_else(|| {
        let f: f64 = token.parse().ok()?;
        let e = (f * 8.0).round();
        (0.0..=320.0)
            .contains(&e)
            .then(|| ChallengeRating::from_eighths(e as u16))
    })
}

fn yes(s: &str) -> bool {
    let s = s.to_lowercase();
    !matches!(s.as_str(), "" | "false" | "no" | "0" | "none" | "-")
}

fn title(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

pub fn kind_of(p: &Profile, v: &Value) -> Kind {
    p.kind_key
        .as_deref()
        .and_then(|k| get(v, k))
        .and_then(text_of)
        .and_then(|value| {
            p.kinds
                .get(&value.to_lowercase())
                .copied()
                .or_else(|| guess_kind(&value))
        })
        .unwrap_or(p.default_kind)
}

pub fn map(p: &Profile, v: &Value) -> Result<Mapped, String> {
    let field = |f: Field| p.fields.get(&f).and_then(|k| get(v, k)).and_then(text_of);
    let name = get(v, &p.name_key)
        .and_then(text_of)
        .ok_or_else(|| format!("no \"{}\"", p.name_key))?;
    let mut kind = kind_of(p, v);
    if kind == Kind::MagicItem && field(Field::Rarity).is_none() {
        kind = Kind::Equipment;
    }
    let (mut facets, _) = default_facets(kind);
    let partial = Cell::new(false);
    let need = |value: Option<String>| -> String {
        match value {
            Some(s) => short(&s),
            None => {
                partial.set(true);
                "Unknown".into()
            }
        }
    };
    match &mut facets {
        Facets::Spell {
            level,
            school,
            classes,
            ritual,
            concentration,
        } => {
            *level = field(Field::Level)
                .as_deref()
                .and_then(level_of)
                .unwrap_or(0)
                .min(9);
            *school = title(&need(field(Field::School)));
            *classes = field(Field::Classes)
                .map(|c| {
                    c.split(',')
                        .map(|x| short(x.trim()))
                        .filter(|x| !x.is_empty())
                        .take(32)
                        .collect()
                })
                .unwrap_or_default();
            *ritual = field(Field::Ritual).as_deref().is_some_and(yes);
            *concentration = field(Field::Concentration).as_deref().is_some_and(yes);
        }
        Facets::Monster {
            cr,
            creature_type,
            size,
        } => {
            match field(Field::Cr).as_deref().and_then(cr_of) {
                Some(c) => *cr = c,
                None => partial.set(true),
            }
            *creature_type = title(&need(field(Field::CreatureType)));
            *size = field(Field::Size)
                .and_then(|s| s.split_whitespace().next().map(title))
                .unwrap_or_else(|| {
                    partial.set(true);
                    "Medium".into()
                });
        }
        Facets::MagicItem {
            rarity,
            category,
            attunement,
        } => {
            *rarity = title(&need(field(Field::Rarity)));
            *category = title(&need(field(Field::Category)));
            *attunement = field(Field::Attunement).as_deref().is_some_and(yes);
        }
        Facets::Equipment { category } | Facets::Feat { category } => {
            *category = title(&need(field(Field::Category)));
        }
        Facets::Subclass { class } => *class = need(field(Field::Class)),
        Facets::Feature { class, level } => {
            *class = need(field(Field::Class));
            *level = field(Field::Level)
                .as_deref()
                .and_then(level_of)
                .unwrap_or(1)
                .clamp(1, 20);
        }
        Facets::Class
        | Facets::Species
        | Facets::Background
        | Facets::Trait
        | Facets::Condition => {}
    }
    let book = p
        .book_key
        .as_deref()
        .and_then(|k| get(v, k))
        .and_then(text_of)
        .or_else(|| p.book.clone().filter(|b| !b.trim().is_empty()))
        .map(|b| b.chars().take(120).collect::<String>());
    let rules = match &book {
        Some(b) if p.rules_from_book => {
            let lower = b.to_lowercase();
            if lower.contains("2024") {
                RulesVersion::R2024
            } else if lower.contains("level up") || lower.contains("a5e") {
                RulesVersion::LevelUp
            } else {
                p.rules
            }
        }
        _ => p.rules,
    };
    let mut used: Vec<&str> = vec![p.name_key.as_str()];
    used.extend(p.text_keys.iter().map(String::as_str));
    used.extend(p.kind_key.as_deref());
    used.extend(p.book_key.as_deref());
    used.extend(p.fields.values().map(String::as_str));
    let mut parts: Vec<String> = p
        .text_keys
        .iter()
        .filter_map(|k| get(v, k).and_then(text_of))
        .collect();
    let extras: Vec<String> = leaf_keys(std::slice::from_ref(v))
        .into_iter()
        .filter(|k| !used.contains(&k.as_str()))
        .filter_map(|k| {
            let value = get(v, &k).and_then(text_of)?;
            Some(format!(
                "- **{}:** {}",
                k.rsplit('.').next().unwrap_or(&k),
                value
            ))
        })
        .take(30)
        .collect();
    if !extras.is_empty() {
        parts.push(extras.join("\n"));
    }
    let text = truncate(&parts.join("\n\n"), MAX_BODY_BYTES);
    Ok(Mapped {
        parsed: Parsed {
            name: NoteTitle::new(&name.chars().take(120).collect::<String>())
                .map_err(|e| format!("name: {e}"))?,
            facets,
            text: NoteBody::new(text).map_err(|e| format!("text: {e}"))?,
            book: book
                .as_deref()
                .map(NoteTitle::new)
                .transpose()
                .map_err(|e| format!("book: {e}"))?,
            rules,
        },
        partial: partial.get(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NESTED: &str = r#"{"meta":{"v":1},"data":{"monsters":[
        {"title":"Goblin","kind":"Monsters","source":"Monster Manual","stats":{"Challenge Rating":"1/4","Size":"Small","Type":"humanoid"},"desc":"Sneaky.","Speed":"30 ft."},
        {"title":"Ogre","kind":"Monsters","source":"Player's Handbook (2024)","stats":{"Challenge Rating":"2","Size":"Large","Type":"giant"},"desc":"Big."}
    ]}}"#;

    #[test]
    fn probe_finds_the_first_list_of_objects_and_stops() {
        let p = probe(NESTED.as_bytes()).unwrap();
        assert_eq!(p.entries, ["data", "monsters"]);
        assert_eq!(p.sample.len(), 2);
        let mut big = String::from("[");
        for i in 0..5000 {
            big.push_str(&format!("{{\"name\":\"e{i}\"}},"));
        }
        big.push_str("oops");
        let p = probe(big.as_bytes()).unwrap();
        assert_eq!(
            (p.entries.len(), p.sample.len()),
            (0, SAMPLE),
            "it stops before the broken tail"
        );
        assert!(probe(&b"{\"a\": 1}"[..]).is_err());
    }

    #[test]
    fn suggested_profile_maps_nested_properties() {
        let p = probe(NESTED.as_bytes()).unwrap();
        let profile = suggest(&p);
        assert_eq!(profile.name_key, "title");
        assert_eq!(profile.text_keys, ["desc"]);
        assert_eq!(profile.kind_key.as_deref(), Some("kind"));
        assert_eq!(profile.kinds.get("monsters"), Some(&Kind::Monster));
        assert_eq!(profile.book_key.as_deref(), Some("source"));
        assert_eq!(
            profile.fields.get(&Field::Cr).map(String::as_str),
            Some("stats.Challenge Rating")
        );
        assert_eq!(
            profile.fields.get(&Field::CreatureType).map(String::as_str),
            Some("stats.Type")
        );
        let mut seen = Vec::new();
        assert!(
            each(NESTED.as_bytes(), &profile.entries, |v| {
                seen.push(map(&profile, &v).unwrap());
                true
            })
            .unwrap()
        );
        assert_eq!(seen.len(), 2);
        let goblin = &seen[0];
        assert!(!goblin.partial);
        assert_eq!(goblin.parsed.rules, RulesVersion::R2014);
        assert_eq!(
            goblin.parsed.facets,
            Facets::Monster {
                cr: ChallengeRating::from_eighths(2),
                creature_type: "Humanoid".into(),
                size: "Small".into()
            }
        );
        assert_eq!(
            goblin.parsed.text.as_str(),
            "Sneaky.\n\n- **Speed:** 30 ft."
        );
        assert_eq!(seen[1].parsed.rules, RulesVersion::R2024);
    }

    #[test]
    fn partly_mapped_entries_are_kept() {
        let profile = Profile {
            name: "test".into(),
            entries: Vec::new(),
            name_key: "name".into(),
            text_keys: Vec::new(),
            kind_key: None,
            kinds: BTreeMap::new(),
            default_kind: Kind::Spell,
            book_key: None,
            book: Some("Homebrew".into()),
            rules: RulesVersion::R2024,
            rules_from_book: false,
            fields: BTreeMap::new(),
        };
        let m = map(&profile, &serde_json::json!({"name": "Zap", "power": 3})).unwrap();
        assert!(m.partial);
        assert!(matches!(m.parsed.facets, Facets::Spell { ref school, .. } if school == "Unknown"));
        assert_eq!(m.parsed.book.as_ref().map(|b| b.as_str()), Some("Homebrew"));
        assert_eq!(m.parsed.text.as_str(), "- **power:** 3");
        assert!(map(&profile, &serde_json::json!({"power": 3})).is_err());
    }

    #[test]
    fn kinds_are_guessed_from_category_names() {
        assert_eq!(guess_kind("Spells"), Some(Kind::Spell));
        assert_eq!(guess_kind("Classes"), Some(Kind::Class));
        assert_eq!(guess_kind("Magic Items"), Some(Kind::MagicItem));
        assert_eq!(guess_kind("Races"), Some(Kind::Species));
        assert_eq!(guess_kind("nonsense"), None);
    }
}
