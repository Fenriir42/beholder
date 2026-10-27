use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use beholder_core::compendium::{
    BUNDLE_FORMAT, Bundle, ChallengeRating, Entry, Facets, GameSystem, Kind, LicenseText,
    RulesVersion, SRD, SourceMeta,
};
use serde_json::Value;

pub const CC_BY: &str = "cc-by-40";

const UPSTREAM: &str = "5e-bits/5e-srd-api@386930c2e93a7f2c9ba2a9715ccc5b95d8926e41";

const ATTRIBUTION: &str = "This work includes material from the System Reference Document 5.2.1 (\"SRD 5.2.1\") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.";

const EXPECTED: [(Kind, usize); 12] = [
    (Kind::Spell, 339),
    (Kind::Monster, 341),
    (Kind::MagicItem, 262),
    (Kind::Equipment, 182),
    (Kind::Class, 12),
    (Kind::Subclass, 12),
    (Kind::Species, 9),
    (Kind::Background, 4),
    (Kind::Feat, 17),
    (Kind::Feature, 232),
    (Kind::Trait, 67),
    (Kind::Condition, 15),
];

pub fn bundle(dir: &Path) -> Result<Vec<u8>> {
    let entries = convert(dir)?;
    let bundle = Bundle {
        format: BUNDLE_FORMAT,
        source: SourceMeta {
            id: SRD.into(),
            name: "System Reference Document 5.2.1".into(),
            publisher: "Wizards of the Coast".into(),
            system: GameSystem::Dnd5e,
            rules: RulesVersion::R2024,
            licenses: vec![CC_BY.into()],
            upstream: UPSTREAM.into(),
            attribution: ATTRIBUTION.into(),
        },
        licenses: vec![LicenseText {
            id: CC_BY.into(),
            name: "Creative Commons Attribution 4.0 International".into(),
            text: "https://creativecommons.org/licenses/by/4.0/legalcode".into(),
        }],
        entries,
    };
    let raw = postcard::to_stdvec(&bundle)?;
    Ok(zstd::bulk::compress(&raw, 19)?)
}

pub fn entries(bundle: &[u8]) -> Result<Vec<Entry>> {
    let raw = zstd::bulk::decompress(bundle, 64 << 20)?;
    Ok(postcard::from_bytes::<Bundle>(&raw)?.entries)
}

fn load(dir: &Path, name: &str) -> Result<Vec<Value>> {
    let path = dir.join(format!("5e-SRD-{name}.json"));
    let text = std::fs::read_to_string(&path).with_context(|| format!("{}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("{}", path.display()))
}

fn convert(dir: &Path) -> Result<Vec<Entry>> {
    let features = load(dir, "Features")?;
    let traits = load(dir, "Traits")?;
    let levels = load(dir, "Levels")?;
    let mut out = Vec::new();
    for (name, f) in [
        ("Spells", spell as fn(&Value) -> Result<Entry>),
        ("Monsters", monster),
        ("Magic-Items", magic_item),
        ("Equipment", equipment),
        ("Subclasses", subclass),
        ("Backgrounds", background),
        ("Feats", feat),
        ("Conditions", condition),
    ] {
        for v in load(dir, name)? {
            out.push(f(&v).with_context(|| format!("{name}: {}", s(&v, "index").unwrap_or("?")))?);
        }
    }
    for v in load(dir, "Classes")? {
        out.push(
            class(&v, &features, &levels)
                .with_context(|| format!("class {}", s(&v, "index").unwrap_or("?")))?,
        );
    }
    for v in load(dir, "Species")? {
        out.push(
            species(&v, &traits)
                .with_context(|| format!("species {}", s(&v, "index").unwrap_or("?")))?,
        );
    }
    for v in &features {
        out.push(feature(v).with_context(|| format!("feature {}", s(v, "index").unwrap_or("?")))?);
    }
    for v in &traits {
        out.push(species_trait(v)?);
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    validate(&out)?;
    Ok(out)
}

fn validate(entries: &[Entry]) -> Result<()> {
    let mut keys = HashSet::new();
    for e in entries {
        ensure!(keys.insert(&e.key), "duplicate key {}", e.key);
        ensure!(!e.name.trim().is_empty(), "{} has no name", e.key);
        ensure!(!e.text.trim().is_empty(), "{} has no text", e.key);
    }
    for (kind, want) in EXPECTED {
        let got = entries.iter().filter(|e| e.facets.kind() == kind).count();
        if got != want {
            bail!("{} count {got}, expected {want}", kind.as_str());
        }
    }
    Ok(())
}

fn s<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v.get(k)
        .and_then(Value::as_str)
        .with_context(|| format!("missing string {k}"))
}

fn opt<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k)
        .and_then(Value::as_str)
        .filter(|t| !t.trim().is_empty())
}

fn arr<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn name_of(v: &Value) -> Option<&str> {
    v.as_str().or_else(|| v.get("name").and_then(Value::as_str))
}

fn names<'a>(v: &'a Value, k: &str) -> Vec<&'a str> {
    arr(v, k).iter().filter_map(name_of).collect()
}

fn key(kind: Kind, v: &Value) -> Result<String> {
    Ok(format!("{}/{}", kind.as_str(), s(v, "index")?))
}

fn paragraphs(text: &str) -> String {
    let mut out = String::new();
    let mut prev_block = false;
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        let l = l
            .strip_prefix("• ")
            .map_or_else(|| l.to_owned(), |r| format!("- {r}"));
        let block = l.starts_with('|') || l.starts_with("- ") || l.starts_with("* ");
        if !out.is_empty() {
            out.push_str(if block && prev_block { "\n" } else { "\n\n" });
        }
        out.push_str(&l);
        prev_block = block;
    }
    out
}

fn text_of(v: &Value, k: &str) -> String {
    match v.get(k) {
        Some(Value::String(t)) => paragraphs(t),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(paragraphs)
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => String::new(),
    }
}

fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut up = true;
    for c in s.chars() {
        if c == '_' || c == ' ' || c == '-' {
            out.push(' ');
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn ordinal(n: u64) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, x) if x != 11 => "st",
        (2, x) if x != 12 => "nd",
        (3, x) if x != 13 => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

fn spell(v: &Value) -> Result<Entry> {
    let level = v.get("level").and_then(Value::as_u64).context("level")?;
    let school = v
        .get("school")
        .and_then(name_of)
        .context("school")?
        .to_owned();
    let classes: Vec<String> = names(v, "classes").into_iter().map(str::to_owned).collect();
    let ritual = v.get("ritual").and_then(Value::as_bool).unwrap_or(false);
    let concentration = v
        .get("concentration")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let mut t = String::new();
    if level == 0 {
        write!(t, "*{school} Cantrip*")?;
    } else {
        write!(t, "*Level {level} {school}*")?;
    }
    if !classes.is_empty() {
        write!(t, " ({})", classes.join(", "))?;
    }
    let mut components = names(v, "components").join(", ");
    if let Some(m) = opt(v, "material") {
        write!(components, " ({m})")?;
    }
    let duration = s(v, "duration")?;
    let duration = if concentration {
        format!("Concentration, {duration}")
    } else {
        title_case_first(duration)
    };
    let casting = s(v, "casting_time")?;
    let casting = if ritual {
        format!("{casting} or Ritual")
    } else {
        casting.to_owned()
    };
    write!(
        t,
        "\n\n| | |\n|---|---|\n| Casting Time | {casting} |\n| Range | {} |\n| Components | {components} |\n| Duration | {duration} |\n\n{}",
        s(v, "range")?,
        text_of(v, "description")
    )?;
    if let Some(h) = opt(v, "higher_level") {
        let label = if level == 0 {
            "Cantrip Upgrade."
        } else {
            "Using a Higher-Level Spell Slot."
        };
        write!(t, "\n\n**{label}** {}", paragraphs(h))?;
    }
    Ok(Entry {
        key: key(Kind::Spell, v)?,
        name: s(v, "name")?.to_owned(),
        facets: Facets::Spell {
            level: u8::try_from(level)?,
            school,
            classes,
            ritual,
            concentration,
        },
        text: t,
    })
}

fn title_case_first(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

fn modifier(score: i64) -> String {
    let m = (score - 10).div_euclid(2);
    if m >= 0 {
        format!("+{m}")
    } else {
        m.to_string()
    }
}

fn cr_eighths(v: &Value) -> Result<u16> {
    let cr = v
        .get("challenge_rating")
        .and_then(Value::as_f64)
        .context("challenge_rating")?;
    let eighths = (cr * 8.0).round();
    ensure!((0.0..=320.0).contains(&eighths), "challenge rating {cr}");
    Ok(eighths as u16)
}

fn monster(v: &Value) -> Result<Entry> {
    let size = s(v, "size")?;
    let creature_type = title_case_first(s(v, "type")?);
    let mut t = format!(
        "*{size} {creature_type}, {}*\n\n",
        title_case(s(v, "alignment")?)
    );

    let ac = arr(v, "armor_class")
        .iter()
        .map(|a| {
            let value = a.get("value").and_then(Value::as_i64).unwrap_or_default();
            let armor = names(a, "armor");
            if armor.is_empty() {
                value.to_string()
            } else {
                format!("{value} ({})", armor.join(", "))
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let hp = v
        .get("hit_points")
        .and_then(Value::as_i64)
        .context("hit_points")?;
    let speed = v
        .get("speed")
        .and_then(Value::as_object)
        .map(|m| {
            let hover = m.get("hover").and_then(Value::as_bool) == Some(true);
            let mut modes: Vec<(&String, &str)> = m
                .iter()
                .filter_map(|(k, val)| val.as_str().map(|d| (k, d)))
                .collect();
            modes.sort_by_key(|(k, _)| k.as_str() != "walk");
            modes
                .into_iter()
                .map(|(k, d)| match k.as_str() {
                    "walk" => d.to_owned(),
                    "fly" if hover => format!("Fly {d} (hover)"),
                    other => format!("{} {d}", title_case(other)),
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    write!(t, "**AC** {ac} · **HP** {hp}")?;
    if let Some(roll) = opt(v, "hit_points_roll") {
        write!(t, " ({roll})")?;
    }
    write!(t, " · **Speed** {speed}\n\n")?;

    let abilities = [
        "strength",
        "dexterity",
        "constitution",
        "intelligence",
        "wisdom",
        "charisma",
    ];
    t.push_str("| STR | DEX | CON | INT | WIS | CHA |\n|---|---|---|---|---|---|\n|");
    for a in abilities {
        let score = v
            .get(a)
            .and_then(Value::as_i64)
            .with_context(|| a.to_owned())?;
        write!(t, " {score} ({}) |", modifier(score))?;
    }
    t.push_str("\n\n");

    let mut lines = Vec::new();
    let profs = |prefix: &str| {
        arr(v, "proficiencies")
            .iter()
            .filter_map(|p| {
                let n = p.get("proficiency").and_then(name_of)?;
                let rest = n.strip_prefix(prefix)?;
                let val = p.get("value").and_then(Value::as_i64)?;
                Some(format!(
                    "{} {}",
                    title_case_first(&rest.to_lowercase()),
                    modifier(10 + 2 * val)
                ))
            })
            .collect::<Vec<_>>()
    };
    let saves = profs("Saving Throw: ");
    if !saves.is_empty() {
        lines.push(format!("**Saving Throws** {}", saves.join(", ")));
    }
    let skills = profs("Skill: ");
    if !skills.is_empty() {
        lines.push(format!("**Skills** {}", skills.join(", ")));
    }
    for (k, label) in [
        ("damage_vulnerabilities", "Vulnerabilities"),
        ("damage_resistances", "Resistances"),
        ("damage_immunities", "Immunities"),
        ("condition_immunities", "Condition Immunities"),
    ] {
        let list = names(v, k);
        if !list.is_empty() {
            let list: Vec<String> = list.into_iter().map(title_case_first).collect();
            lines.push(format!("**{label}** {}", list.join(", ")));
        }
    }
    if let Some(g) = opt(v, "gear") {
        lines.push(format!("**Gear** {g}"));
    }
    if let Some(senses) = v.get("senses").and_then(Value::as_object) {
        let mut parts: Vec<String> = senses
            .iter()
            .filter(|(k, _)| k.as_str() != "passive_perception")
            .filter_map(|(k, d)| d.as_str().map(|d| format!("{} {d}", title_case(k))))
            .collect();
        if let Some(pp) = senses.get("passive_perception").and_then(Value::as_i64) {
            parts.push(format!("Passive Perception {pp}"));
        }
        lines.push(format!("**Senses** {}", parts.join(", ")));
    }
    lines.push(format!(
        "**Languages** {}",
        opt(v, "languages").unwrap_or("None")
    ));
    let cr = ChallengeRating::from_eighths(cr_eighths(v)?);
    let mut cr_line = format!("**CR** {cr}");
    if let Some(xp) = v.get("xp").and_then(Value::as_i64) {
        write!(cr_line, " (XP {xp}")?;
        if let Some(lair) = v.get("xp_in_lair").and_then(Value::as_i64) {
            write!(cr_line, ", or {lair} in lair")?;
        }
        if let Some(pb) = v.get("proficiency_bonus").and_then(Value::as_i64) {
            write!(cr_line, "; PB +{pb}")?;
        }
        cr_line.push(')');
    }
    lines.push(cr_line);
    t.push_str(&lines.join("  \n"));

    for (k, heading) in [
        ("special_abilities", "Traits"),
        ("actions", "Actions"),
        ("bonus_actions", "Bonus Actions"),
        ("reactions", "Reactions"),
        ("legendary_actions", "Legendary Actions"),
    ] {
        let items = arr(v, k);
        if items.is_empty() {
            continue;
        }
        write!(t, "\n\n## {heading}")?;
        for a in items {
            let n = a.get("name").and_then(Value::as_str).unwrap_or_default();
            let usage = a
                .get("usage")
                .and_then(|u| {
                    let kind = u.get("type")?.as_str()?;
                    Some(match (kind, u.get("times").and_then(Value::as_i64)) {
                        ("recharge on roll", _) => format!(
                            " (Recharge {}–6)",
                            u.get("min_value").and_then(Value::as_i64).unwrap_or(6)
                        ),
                        ("per day", Some(n)) => format!(" ({n}/Day)"),
                        _ => String::new(),
                    })
                })
                .unwrap_or_default();
            write!(t, "\n\n**{n}{usage}.** {}", text_of(a, "desc"))?;
            if let Some(sc) = a.get("spellcasting") {
                t.push_str(&monster_spells(sc));
            }
        }
    }
    let forms = names(v, "forms");
    if !forms.is_empty() {
        write!(t, "\n\n*Other forms:* {}", forms.join(", "))?;
    }

    Ok(Entry {
        key: key(Kind::Monster, v)?,
        name: s(v, "name")?.to_owned(),
        facets: Facets::Monster {
            cr,
            creature_type,
            size: size.to_owned(),
        },
        text: t,
    })
}

fn monster_spells(sc: &Value) -> String {
    let mut groups: Vec<(String, Vec<&str>)> = Vec::new();
    for sp in arr(sc, "spells") {
        let Some(n) = name_of(sp) else { continue };
        let usage = sp.get("usage");
        let label = match (
            usage.and_then(|u| u.get("type")).and_then(Value::as_str),
            usage.and_then(|u| u.get("times")).and_then(Value::as_i64),
        ) {
            (Some("per day"), Some(times)) => format!("{times}/Day each"),
            (Some(other), _) => title_case_first(other),
            (None, _) => "Spells".to_owned(),
        };
        match groups.iter_mut().find(|(l, _)| *l == label) {
            Some((_, list)) => list.push(n),
            None => groups.push((label, vec![n])),
        }
    }
    groups
        .into_iter()
        .map(|(label, list)| format!("\n- **{label}:** {}", list.join(", ")))
        .collect::<String>()
        .replacen("\n-", "\n\n-", 1)
}

fn magic_item(v: &Value) -> Result<Entry> {
    let rarity = v
        .get("rarity")
        .and_then(name_of)
        .context("rarity")?
        .to_owned();
    let category = v
        .get("equipment_category")
        .and_then(name_of)
        .context("equipment_category")?
        .to_owned();
    let attunement = v
        .get("attunement")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let desc: Vec<&str> = arr(v, "desc").iter().filter_map(Value::as_str).collect();
    let (first, rest) = desc.split_first().context("desc")?;
    let mut t = format!("*{}*\n\n**Rarity** {rarity}", first.trim());
    if attunement {
        t.push_str(" · **Requires attunement**");
        if let Some(by) = opt(v, "limited-to") {
            write!(t, " by a {by}")?;
        }
    }
    for p in rest {
        write!(t, "\n\n{}", paragraphs(p))?;
    }
    let variants = names(v, "variants");
    if !variants.is_empty() {
        write!(t, "\n\n*Variants:* {}", variants.join(", "))?;
    }
    Ok(Entry {
        key: key(Kind::MagicItem, v)?,
        name: s(v, "name")?.to_owned(),
        facets: Facets::MagicItem {
            rarity,
            category,
            attunement,
        },
        text: t,
    })
}

const TOP_CATEGORIES: [&str; 9] = [
    "Weapons",
    "Armor",
    "Tools",
    "Ammunition",
    "Equipment Packs",
    "Arcane Foci",
    "Druidic Foci",
    "Holy Symbols",
    "Adventuring Gear",
];

fn equipment(v: &Value) -> Result<Entry> {
    let cats = names(v, "equipment_categories");
    let category = TOP_CATEGORIES
        .iter()
        .find(|c| cats.contains(c))
        .copied()
        .or_else(|| cats.first().copied())
        .context("equipment_categories")?
        .to_owned();
    let mut rows = Vec::new();
    let detail: Vec<&str> = cats.iter().copied().filter(|c| *c != category).collect();
    if !detail.is_empty() {
        rows.push(("Type", detail.join(", ")));
    }
    if let Some(c) = v.get("cost") {
        let q = c
            .get("quantity")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        rows.push(("Cost", format!("{q} {}", s(c, "unit").unwrap_or("gp"))));
    }
    if let Some(w) = v.get("weight").and_then(Value::as_f64) {
        rows.push(("Weight", format!("{w} lb.")));
    }
    if let Some(d) = v.get("damage") {
        rows.push((
            "Damage",
            format!(
                "{} {}",
                s(d, "damage_dice").unwrap_or_default(),
                d.get("damage_type").and_then(name_of).unwrap_or_default()
            ),
        ));
    }
    if let Some(d) = v.get("two_handed_damage") {
        rows.push((
            "Two-Handed",
            format!(
                "{} {}",
                s(d, "damage_dice").unwrap_or_default(),
                d.get("damage_type").and_then(name_of).unwrap_or_default()
            ),
        ));
    }
    let props = names(v, "properties");
    if !props.is_empty() {
        rows.push(("Properties", props.join(", ")));
    }
    if let Some(m) = v.get("mastery").and_then(name_of) {
        rows.push(("Mastery", m.to_owned()));
    }
    if let Some(r) = v.get("range") {
        let normal = r.get("normal").and_then(Value::as_i64);
        let long = r.get("long").and_then(Value::as_i64);
        if let (Some(n), Some(l)) = (normal, long) {
            rows.push(("Range", format!("{n}/{l} ft.")));
        }
    }
    if let Some(ac) = v.get("armor_class") {
        let base = ac.get("base").and_then(Value::as_i64).unwrap_or_default();
        let dex = ac.get("dex_bonus").and_then(Value::as_bool) == Some(true);
        let max = ac.get("max_bonus").and_then(Value::as_i64);
        let text = match (dex, max) {
            (true, Some(m)) => format!("{base} + Dex modifier (max {m})"),
            (true, None) => format!("{base} + Dex modifier"),
            (false, _) if category == "Armor" && cats.contains(&"Shields") => format!("+{base}"),
            (false, _) => base.to_string(),
        };
        rows.push(("Armor Class", text));
    }
    if let Some(st) = v
        .get("str_minimum")
        .and_then(Value::as_i64)
        .filter(|n| *n > 0)
    {
        rows.push(("Strength", format!("{st}+")));
    }
    if v.get("stealth_disadvantage").and_then(Value::as_bool) == Some(true) {
        rows.push(("Stealth", "Disadvantage".to_owned()));
    }
    let mut t = format!("*{category}*");
    if !rows.is_empty() {
        t.push_str("\n\n| | |\n|---|---|");
        for (k, val) in rows {
            write!(t, "\n| {k} | {val} |")?;
        }
    }
    let desc = text_of(v, "description");
    if !desc.is_empty() {
        write!(t, "\n\n{desc}")?;
    }
    let contents: Vec<String> = arr(v, "contents")
        .iter()
        .filter_map(|c| {
            let item = c.get("item").and_then(name_of)?;
            let q = c.get("quantity").and_then(Value::as_i64).unwrap_or(1);
            Some(if q > 1 {
                format!("{item} ×{q}")
            } else {
                item.to_owned()
            })
        })
        .collect();
    if !contents.is_empty() {
        write!(t, "\n\n**Contains** {}", contents.join(", "))?;
    }
    Ok(Entry {
        key: key(Kind::Equipment, v)?,
        name: s(v, "name")?.to_owned(),
        facets: Facets::Equipment { category },
        text: t,
    })
}

fn level_number(v: &Value) -> Result<u8> {
    let level = v.get("level").context("level")?;
    if let Some(n) = level.as_u64() {
        return Ok(u8::try_from(n)?);
    }
    let index = s(level, "index")?;
    let n = index.rsplit('-').next().context("level index")?;
    Ok(n.parse()?)
}

fn class(v: &Value, features: &[Value], levels: &[Value]) -> Result<Entry> {
    let name = s(v, "name")?;
    let index = s(v, "index")?;
    let mut t = String::new();
    let mut facts = vec![format!(
        "**Hit Die** d{}",
        v.get("hit_die")
            .and_then(Value::as_u64)
            .context("hit_die")?
    )];
    if let Some(p) = v
        .get("primary_ability")
        .and_then(|p| p.get("desc"))
        .and_then(Value::as_str)
    {
        facts.push(format!("**Primary Ability** {p}"));
    }
    let saves = names(v, "saving_throws");
    if !saves.is_empty() {
        facts.push(format!("**Saving Throws** {}", saves.join(", ")));
    }
    let profs = names(v, "proficiencies");
    if !profs.is_empty() {
        facts.push(format!("**Proficiencies** {}", profs.join(", ")));
    }
    for c in arr(v, "proficiency_choices") {
        if let Some(d) = opt(c, "desc") {
            facts.push(format!("**Skills** {d}"));
        }
    }
    for c in arr(v, "starting_equipment_options") {
        if let Some(d) = opt(c, "desc") {
            facts.push(format!("**Starting Equipment** {d}"));
        }
    }
    t.push_str(&facts.join("  \n"));

    let mut rows: Vec<&Value> = levels
        .iter()
        .filter(|l| {
            l.get("class")
                .and_then(|c| c.get("index"))
                .and_then(Value::as_str)
                == Some(index)
        })
        .filter(|l| l.get("subclass").is_none())
        .collect();
    rows.sort_by_key(|l| l.get("level").and_then(Value::as_u64));
    if !rows.is_empty() {
        let mut extra: Vec<String> = Vec::new();
        for l in &rows {
            for section in ["class_specific", "spellcasting"] {
                if let Some(m) = l.get(section).and_then(Value::as_object) {
                    for (k, val) in m {
                        if val.is_number() && !extra.contains(k) {
                            extra.push(k.clone());
                        }
                    }
                }
            }
        }
        write!(t, "\n\n## {name} Features\n\n| Level | PB | Features |")?;
        for k in &extra {
            write!(t, " {} |", column_label(k))?;
        }
        t.push_str("\n|---|---|---|");
        for _ in &extra {
            t.push_str("---|");
        }
        for l in &rows {
            let lv = l.get("level").and_then(Value::as_u64).unwrap_or_default();
            let pb = l
                .get("prof_bonus")
                .and_then(Value::as_i64)
                .unwrap_or_default();
            write!(
                t,
                "\n| {lv} | +{pb} | {} |",
                names(l, "features").join(", ")
            )?;
            for k in &extra {
                let val = ["class_specific", "spellcasting"]
                    .iter()
                    .find_map(|sec| l.get(*sec).and_then(|m| m.get(k)))
                    .map(|n| n.to_string())
                    .filter(|n| n != "0")
                    .unwrap_or_else(|| "—".to_owned());
                write!(t, " {val} |")?;
            }
        }
    }

    let mut own: Vec<(u8, &Value)> = features
        .iter()
        .filter(|f| {
            f.get("class")
                .and_then(|c| c.get("index"))
                .and_then(Value::as_str)
                == Some(index)
        })
        .filter(|f| f.get("subclass").is_none())
        .map(|f| Ok((level_number(f)?, f)))
        .collect::<Result<_>>()?;
    own.sort_by_key(|(lv, _)| *lv);
    for (lv, f) in own {
        write!(
            t,
            "\n\n### Level {lv}: {}\n\n{}",
            s(f, "name")?,
            text_of(f, "description")
        )?;
    }
    let subclasses = names(v, "subclasses");
    if !subclasses.is_empty() {
        write!(t, "\n\n**Subclasses** {}", subclasses.join(", "))?;
    }
    Ok(Entry {
        key: key(Kind::Class, v)?,
        name: name.to_owned(),
        facets: Facets::Class,
        text: t,
    })
}

fn column_label(k: &str) -> String {
    if let Some(n) = k.strip_prefix("spell_slots_level_") {
        return n.parse().map_or_else(|_| title_case(k), ordinal);
    }
    title_case(k)
}

fn subclass(v: &Value) -> Result<Entry> {
    let class = v
        .get("class")
        .and_then(name_of)
        .context("class")?
        .to_owned();
    let mut t = String::new();
    if let Some(sum) = opt(v, "summary") {
        write!(t, "*{class} subclass: {sum}*\n\n")?;
    } else {
        write!(t, "*{class} subclass*\n\n")?;
    }
    t.push_str(&text_of(v, "description"));
    let mut feats: BTreeMap<(u8, String), String> = BTreeMap::new();
    for f in arr(v, "features") {
        feats.insert(
            (level_number(f)?, s(f, "name")?.to_owned()),
            text_of(f, "description"),
        );
    }
    for ((lv, n), d) in feats {
        write!(t, "\n\n### Level {lv}: {n}\n\n{d}")?;
    }
    Ok(Entry {
        key: key(Kind::Subclass, v)?,
        name: s(v, "name")?.to_owned(),
        facets: Facets::Subclass { class },
        text: t,
    })
}

fn species(v: &Value, traits: &[Value]) -> Result<Entry> {
    let mut facts = vec![format!("**Creature Type** {}", s(v, "type")?)];
    let size = match opt(v, "size") {
        Some(size) => size.to_owned(),
        None => {
            let options: Vec<&str> = v
                .get("size_options")
                .and_then(|o| o.get("from"))
                .map(|f| arr(f, "options"))
                .unwrap_or_default()
                .iter()
                .filter_map(|o| o.get("size").and_then(Value::as_str))
                .collect();
            ensure!(!options.is_empty(), "size");
            format!(
                "{} (chosen when you select this species)",
                options.join(" or ")
            )
        }
    };
    facts.push(format!("**Size** {size}"));
    if let Some(sp) = v.get("speed").and_then(Value::as_i64) {
        facts.push(format!("**Speed** {sp} feet"));
    }
    let mut t = facts.join("  \n");
    for r in arr(v, "traits") {
        let idx = s(r, "index")?;
        let tr = traits
            .iter()
            .find(|tr| s(tr, "index").ok() == Some(idx))
            .with_context(|| format!("trait {idx}"))?;
        write!(
            t,
            "\n\n**{}.** {}",
            s(tr, "name")?,
            text_of(tr, "description")
        )?;
    }
    let sub = names(v, "subspecies");
    if !sub.is_empty() {
        write!(t, "\n\n**Options** {}", sub.join(", "))?;
    }
    Ok(Entry {
        key: key(Kind::Species, v)?,
        name: s(v, "name")?.to_owned(),
        facets: Facets::Species,
        text: t,
    })
}

fn species_trait(v: &Value) -> Result<Entry> {
    let of = names(v, "species");
    let mut t = String::new();
    if !of.is_empty() {
        write!(t, "*{} trait*\n\n", of.join(", "))?;
    }
    t.push_str(&text_of(v, "description"));
    Ok(Entry {
        key: key(Kind::Trait, v)?,
        name: s(v, "name")?.to_owned(),
        facets: Facets::Trait,
        text: t,
    })
}

fn background(v: &Value) -> Result<Entry> {
    let mut facts = Vec::new();
    let scores = names(v, "ability_scores");
    if !scores.is_empty() {
        facts.push(format!("**Ability Scores** {}", scores.join(", ")));
    }
    if let Some(f) = v.get("feat") {
        let n = name_of(f).context("feat")?;
        match opt(f, "note") {
            Some(note) => facts.push(format!("**Feat** {n} ({note})")),
            None => facts.push(format!("**Feat** {n}")),
        }
    }
    let profs = names(v, "proficiencies");
    if !profs.is_empty() {
        facts.push(format!("**Proficiencies** {}", profs.join(", ")));
    }
    for c in arr(v, "proficiency_choices") {
        if let Some(d) = opt(c, "desc") {
            facts.push(format!("**Choose** {d}"));
        }
    }
    for c in arr(v, "equipment_options") {
        if let Some(d) = opt(c, "desc") {
            facts.push(format!("**Equipment** {d}"));
        }
    }
    Ok(Entry {
        key: key(Kind::Background, v)?,
        name: s(v, "name")?.to_owned(),
        facets: Facets::Background,
        text: facts.join("  \n"),
    })
}

fn feat(v: &Value) -> Result<Entry> {
    let category = title_case(s(v, "type")?);
    let mut t = format!("*{category} Feat*");
    let mut pre = Vec::new();
    for p in arr(v, "prerequisites") {
        if let Some(l) = p.get("minimum_level").and_then(Value::as_u64) {
            pre.push(format!("Level {l}+"));
        }
    }
    for p in arr(v, "prerequisite_options") {
        if let Some(d) = opt(p, "desc") {
            pre.push(d.to_owned());
        }
    }
    if !pre.is_empty() {
        write!(t, " (Prerequisite: {})", pre.join("; "))?;
    }
    if v.get("repeatable").and_then(Value::as_bool) == Some(true) {
        t.push_str(" · Repeatable");
    }
    write!(t, "\n\n{}", text_of(v, "description"))?;
    Ok(Entry {
        key: key(Kind::Feat, v)?,
        name: s(v, "name")?.to_owned(),
        facets: Facets::Feat { category },
        text: t,
    })
}

fn feature(v: &Value) -> Result<Entry> {
    let class = v
        .get("class")
        .and_then(name_of)
        .context("class")?
        .to_owned();
    let level = level_number(v)?;
    let header = match v.get("subclass").and_then(name_of) {
        Some(sub) => format!("*{class} ({sub}) feature, level {level}*"),
        None => format!("*{class} feature, level {level}*"),
    };
    Ok(Entry {
        key: key(Kind::Feature, v)?,
        name: s(v, "name")?.to_owned(),
        facets: Facets::Feature { class, level },
        text: format!("{header}\n\n{}", text_of(v, "description")),
    })
}

fn condition(v: &Value) -> Result<Entry> {
    Ok(Entry {
        key: key(Kind::Condition, v)?,
        name: s(v, "name")?.to_owned(),
        facets: Facets::Condition,
        text: text_of(v, "description"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraphs_split_and_keep_tables() {
        assert_eq!(paragraphs("a\n  b\n\nc"), "a\n\nb\n\nc");
        assert_eq!(
            paragraphs("intro\n| x | y |\n|---|---|\n| 1 | 2 |\nafter"),
            "intro\n\n| x | y |\n|---|---|\n| 1 | 2 |\n\nafter"
        );
        assert_eq!(paragraphs("• one\n• two"), "- one\n- two");
    }

    #[test]
    fn small_helpers() {
        assert_eq!(modifier(15), "+2");
        assert_eq!(modifier(9), "-1");
        assert_eq!(modifier(10), "+0");
        assert_eq!(ordinal(1), "1st");
        assert_eq!(ordinal(3), "3rd");
        assert_eq!(ordinal(11), "11th");
        assert_eq!(title_case("passive_perception"), "Passive Perception");
        assert_eq!(column_label("spell_slots_level_2"), "2nd");
    }

    #[test]
    fn srd_converts_and_is_reproducible() {
        let dir = crate::root().join("data/srd-5.2/upstream");
        let a = bundle(&dir).unwrap();
        let b = bundle(&dir).unwrap();
        assert_eq!(a, b, "bundles must be byte-identical across runs");
        let raw = zstd::bulk::decompress(&a, 64 << 20).unwrap();
        let back: Bundle = postcard::from_bytes(&raw).unwrap();
        let fireball = back
            .entries
            .iter()
            .find(|e| e.key == "spell/fireball")
            .unwrap();
        assert!(matches!(fireball.facets, Facets::Spell { level: 3, .. }));
        assert!(fireball.text.contains("Casting Time"));
        let dragon = back
            .entries
            .iter()
            .find(|e| e.key == "monster/adult-red-dragon")
            .unwrap();
        assert!(
            dragon.text.contains("## Legendary Actions"),
            "{}",
            dragon.text
        );
    }
}
