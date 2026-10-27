use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};
use beholder_core::compendium::{
    BUNDLE_FORMAT, Bundle, ChallengeRating, Entry, Facets, GameSystem, Kind, LicenseText,
    RulesVersion, SourceMeta,
};
use serde_json::Value;

pub const COMMIT: &str = "e69ac13b49dae6a9d84abf38fc674fe1db5b392b";

const REPO: &str = "https://github.com/open5e/open5e-api.git";

const SKIP: [&str; 3] = ["srd-2024", "tob", "elderberry-inn-icons"];

pub fn checkout(root: &Path) -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("OPEN5E_DIR") {
        return Ok(PathBuf::from(dir).join("data/v2"));
    }
    let dir = root.join(format!("target/open5e/{COMMIT}"));
    let v2 = dir.join("data/v2");
    if v2.join("License.json").exists() {
        return Ok(v2);
    }
    std::fs::create_dir_all(&dir)?;
    let git = |args: &[&str]| -> Result<()> {
        let ok = Command::new("git")
            .args(args)
            .current_dir(&dir)
            .status()
            .context("running git")?
            .success();
        ensure!(ok, "git {} failed", args.join(" "));
        Ok(())
    };
    git(&["init", "-q"])?;
    git(&["remote", "add", "origin", REPO]).ok();
    git(&["sparse-checkout", "set", "--no-cone", "data/v2/"])?;
    git(&[
        "fetch",
        "-q",
        "--depth",
        "1",
        "--filter=blob:none",
        "origin",
        COMMIT,
    ])?;
    git(&["checkout", "-q", "FETCH_HEAD"])?;
    Ok(v2)
}

struct Rec {
    pk: String,
    doc: String,
    fields: Value,
}

#[derive(Default)]
struct Data {
    models: HashMap<String, Vec<Rec>>,
    publishers: HashMap<String, String>,
    docs: BTreeMap<String, (String, Value)>,
}

impl Data {
    fn load(v2: &Path) -> Result<Self> {
        let mut data = Self::default();
        for publisher in sorted_dirs(v2)? {
            let pub_id = name_of_path(&publisher);
            let pub_file = publisher.join("Publisher.json");
            if let Ok(t) = std::fs::read_to_string(&pub_file) {
                let v: Vec<Value> = serde_json::from_str(&t)?;
                if let Some(n) = v.first().and_then(|p| p["fields"]["name"].as_str()) {
                    data.publishers.insert(pub_id.clone(), n.to_owned());
                }
            }
            for doc in sorted_dirs(&publisher)? {
                let doc_id = name_of_path(&doc);
                let mut files: Vec<PathBuf> = std::fs::read_dir(&doc)?
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().is_some_and(|x| x == "json"))
                    .collect();
                files.sort();
                for f in files {
                    let model = f
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .context("file name")?
                        .to_owned();
                    let text = std::fs::read_to_string(&f)?;
                    let rows: Vec<Value> =
                        serde_json::from_str(&text).with_context(|| f.display().to_string())?;
                    if model == "Document" {
                        if let Some(d) = rows.into_iter().next() {
                            data.docs
                                .insert(doc_id.clone(), (pub_id.clone(), d["fields"].clone()));
                        }
                        continue;
                    }
                    let list = data.models.entry(model).or_default();
                    for r in rows {
                        let pk = match &r["pk"] {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        };
                        list.push(Rec {
                            pk,
                            doc: doc_id.clone(),
                            fields: r["fields"].clone(),
                        });
                    }
                }
            }
        }
        for list in data.models.values_mut() {
            list.sort_by(|a, b| a.pk.cmp(&b.pk));
        }
        Ok(data)
    }

    fn all(&self, model: &str) -> &[Rec] {
        self.models.get(model).map_or(&[], Vec::as_slice)
    }

    fn of_doc<'a>(&'a self, model: &str, doc: &'a str) -> impl Iterator<Item = &'a Rec> + 'a {
        self.all(model).iter().filter(move |r| r.doc == doc)
    }

    fn name(&self, model: &str, pk: &str) -> Option<&str> {
        self.all(model)
            .iter()
            .find(|r| r.pk == pk)
            .and_then(|r| r.fields["name"].as_str())
    }

    fn children(&self, model: &str, parent: &str) -> Vec<&Rec> {
        self.all(model)
            .iter()
            .filter(|r| r.fields["parent"].as_str() == Some(parent))
            .collect()
    }
}

fn sorted_dirs(p: &Path) -> Result<Vec<PathBuf>> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(p)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    Ok(v)
}

fn name_of_path(p: &Path) -> String {
    p.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_owned()
}

pub fn bundles(v2: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let data = Data::load(v2)?;
    let licenses: HashMap<String, LicenseText> = {
        let text = std::fs::read_to_string(v2.join("License.json"))?;
        let rows: Vec<Value> = serde_json::from_str(&text)?;
        rows.into_iter()
            .filter_map(|r| {
                let id = r["pk"].as_str()?.to_owned();
                Some((
                    id.clone(),
                    LicenseText {
                        id,
                        name: r["fields"]["name"].as_str()?.to_owned(),
                        text: r["fields"]["desc"].as_str()?.to_owned(),
                    },
                ))
            })
            .collect()
    };
    let mut out = Vec::new();
    for (doc, (publisher_id, meta)) in &data.docs {
        if SKIP.contains(&doc.as_str()) {
            continue;
        }
        let entries = convert(&data, doc).with_context(|| format!("document {doc}"))?;
        if entries.is_empty() {
            continue;
        }
        let publisher = data
            .publishers
            .get(publisher_id)
            .cloned()
            .unwrap_or_else(|| publisher_id.clone());
        let ids: Vec<String> = meta["licenses"]
            .as_array()
            .map(|l| {
                l.iter()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let source = SourceMeta {
            id: doc.clone(),
            name: meta["name"].as_str().context("document name")?.to_owned(),
            publisher: publisher.clone(),
            system: GameSystem::Dnd5e,
            rules: match meta["gamesystem"].as_str() {
                Some("5e-2024") => RulesVersion::R2024,
                Some("a5e") => RulesVersion::LevelUp,
                _ => RulesVersion::R2014,
            },
            licenses: ids.clone(),
            upstream: format!("open5e/open5e-api@{COMMIT} (data/v2/{publisher_id}/{doc})"),
            attribution: notice(meta, &publisher, &ids),
        };
        let bundle = Bundle {
            seq: 0,
            format: BUNDLE_FORMAT,
            source,
            licenses: ids
                .iter()
                .filter_map(|i| licenses.get(i).cloned())
                .collect(),
            entries,
        };
        let raw = postcard::to_stdvec(&bundle)?;
        out.push((doc.clone(), zstd::bulk::compress(&raw, 19)?));
    }
    Ok(out)
}

fn notice(meta: &Value, publisher: &str, licenses: &[String]) -> String {
    let name = meta["name"].as_str().unwrap_or_default();
    let author = meta["author"].as_str().unwrap_or_default().trim();
    let year = meta["publication_date"]
        .as_str()
        .and_then(|d| d.get(..4))
        .unwrap_or_default();
    let mut lines = Vec::new();
    if licenses.iter().any(|l| l == "ogl-10a") {
        let mut s = format!("{name}. Copyright {year}, {publisher}.");
        if !author.is_empty() {
            write!(s, " Authors: {author}.").ok();
        }
        lines.push(s);
    }
    if licenses.iter().any(|l| l == "cc-by-40") {
        let by = if author.is_empty() {
            publisher.to_owned()
        } else {
            format!("{author} ({publisher})")
        };
        lines.push(format!(
            "This work includes material from {name} by {by}, licensed under the Creative Commons Attribution 4.0 International License (https://creativecommons.org/licenses/by/4.0/legalcode)."
        ));
    }
    if licenses.iter().any(|l| l == "cc0") {
        lines.push(format!(
            "{name} by {publisher} is dedicated to the public domain (CC0 1.0)."
        ));
    }
    lines.join("\n\n")
}

fn convert(data: &Data, doc: &str) -> Result<Vec<Entry>> {
    let mut out = Vec::new();
    for r in data.of_doc("Creature", doc) {
        out.push(creature(data, r).with_context(|| r.pk.clone())?);
    }
    for r in data.of_doc("Spell", doc) {
        out.push(spell(data, r).with_context(|| r.pk.clone())?);
    }
    for r in data.of_doc("MagicItem", doc) {
        out.push(magic_item(data, r)?);
    }
    for r in data.of_doc("Item", doc) {
        out.push(item(data, r)?);
    }
    for r in data.of_doc("Feat", doc) {
        out.push(feat(data, r)?);
    }
    for r in data.of_doc("Background", doc) {
        out.push(background(data, r)?);
    }
    for r in data.of_doc("CharacterClass", doc) {
        out.extend(class(data, r).with_context(|| r.pk.clone())?);
    }
    for r in data.of_doc("Species", doc) {
        out.push(species(data, r)?);
    }
    for r in data.of_doc("ConditionDescription", doc) {
        out.push(condition(data, r)?);
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    let mut keys = HashSet::new();
    for e in &out {
        ensure!(keys.insert(e.key.clone()), "duplicate key {}", e.key);
        ensure!(!e.name.trim().is_empty(), "{} has no name", e.key);
        ensure!(!e.text.trim().is_empty(), "{} has no text", e.key);
    }
    Ok(out)
}

fn s<'a>(r: &'a Rec, k: &str) -> Option<&'a str> {
    r.fields[k].as_str().filter(|t| !t.trim().is_empty())
}

fn name(r: &Rec) -> Result<String> {
    Ok(s(r, "name")
        .with_context(|| format!("{} has no name", r.pk))?
        .trim()
        .to_owned())
}

fn key(kind: Kind, r: &Rec) -> String {
    let slug = r.pk.strip_prefix(&format!("{}_", r.doc)).unwrap_or(&r.pk);
    format!("{}/{slug}", kind.as_str())
}

fn title(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut up = true;
    for c in s.chars() {
        if c == '_' || c == '-' || c == ' ' {
            out.push(' ');
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

fn first_upper(s: &str) -> String {
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

fn signed(n: i64) -> String {
    if n >= 0 {
        format!("+{n}")
    } else {
        n.to_string()
    }
}

fn num(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}

fn trim_num(f: f64) -> String {
    let s = format!("{f:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn creature(data: &Data, r: &Rec) -> Result<Entry> {
    let f = &r.fields;
    let size = title(s(r, "size").unwrap_or("medium"));
    let creature_type = title(s(r, "type").unwrap_or("creature"));
    let mut t = format!("*{size} {creature_type}");
    if let Some(sub) = s(r, "subcategory") {
        write!(t, " ({sub})")?;
    }
    write!(
        t,
        ", {}*\n\n",
        title(s(r, "alignment").unwrap_or("unaligned"))
    )?;

    let mut ac = f["armor_class"].as_i64().unwrap_or_default().to_string();
    if let Some(d) = s(r, "armor_detail") {
        write!(ac, " ({d})")?;
    }
    let mut hp = f["hit_points"].as_i64().unwrap_or_default().to_string();
    if let Some(d) = s(r, "hit_dice") {
        write!(hp, " ({d})")?;
    }
    let unit = s(r, "unit").unwrap_or("ft.");
    let unit = if unit == "feet" { "ft." } else { unit };
    let mut speeds = Vec::new();
    for mode in ["walk", "burrow", "climb", "fly", "swim"] {
        if let Some(v) = num(&f[mode]).filter(|v| *v > 0.0) {
            let mut sp = if mode == "walk" {
                format!("{} {unit}", trim_num(v))
            } else {
                format!("{} {} {unit}", title(mode), trim_num(v))
            };
            if mode == "fly" && f["hover"].as_bool() == Some(true) {
                sp.push_str(" (hover)");
            }
            speeds.push(sp);
        }
    }
    write!(
        t,
        "**AC** {ac} · **HP** {hp} · **Speed** {}\n\n",
        speeds.join(", ")
    )?;

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
        let score = f[format!("ability_score_{a}")].as_i64().unwrap_or(10);
        write!(t, " {score} ({}) |", modifier(score))?;
    }
    t.push_str("\n\n");

    let mut lines = Vec::new();
    let saves: Vec<String> = abilities
        .iter()
        .filter_map(|a| {
            let v = f[format!("saving_throw_{a}")].as_i64()?;
            Some(format!("{} {}", title(&a[..3]), signed(v)))
        })
        .collect();
    if !saves.is_empty() {
        lines.push(format!("**Saving Throws** {}", saves.join(", ")));
    }
    let mut skills: Vec<String> = f
        .as_object()
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| {
                    let skill = k.strip_prefix("skill_bonus_")?;
                    Some(format!("{} {}", title(skill), signed(v.as_i64()?)))
                })
                .collect()
        })
        .unwrap_or_default();
    skills.sort();
    if !skills.is_empty() {
        lines.push(format!("**Skills** {}", skills.join(", ")));
    }
    for (k, label) in [
        ("damage_vulnerabilities_display", "Vulnerabilities"),
        ("damage_resistances_display", "Resistances"),
        ("damage_immunities_display", "Immunities"),
        ("condition_immunities_display", "Condition Immunities"),
    ] {
        if let Some(v) = s(r, k) {
            lines.push(format!("**{label}** {}", first_upper(v)));
        }
    }
    let mut senses = Vec::new();
    for (k, label) in [
        ("blindsight_range", "Blindsight"),
        ("darkvision_range", "Darkvision"),
        ("tremorsense_range", "Tremorsense"),
        ("truesight_range", "Truesight"),
        ("telepathy_range", "Telepathy"),
    ] {
        if let Some(v) = num(&f[k]).filter(|v| *v > 0.0) {
            senses.push(format!("{label} {} {unit}", trim_num(v)));
        }
    }
    if let Some(pp) = f["passive_perception"].as_i64() {
        senses.push(format!("Passive Perception {pp}"));
    }
    if !senses.is_empty() {
        lines.push(format!("**Senses** {}", senses.join(", ")));
    }
    lines.push(format!(
        "**Languages** {}",
        s(r, "languages_desc").unwrap_or("—")
    ));
    let cr_value = num(&f["challenge_rating"]).unwrap_or_default();
    let eighths = (cr_value * 8.0).round();
    ensure!(
        (0.0..=320.0).contains(&eighths),
        "challenge rating {cr_value}"
    );
    let cr = ChallengeRating::from_eighths(eighths as u16);
    let mut cr_line = format!("**CR** {cr}");
    if let Some(xp) = f["experience_points_integer"].as_i64() {
        write!(cr_line, " (XP {xp})")?;
    }
    lines.push(cr_line);
    t.push_str(&lines.join("  \n"));

    let traits = data.children("CreatureTrait", &r.pk);
    if !traits.is_empty() {
        t.push_str("\n\n## Traits");
        for tr in traits {
            write!(
                t,
                "\n\n**{}.** {}",
                name(tr)?,
                s(tr, "desc").unwrap_or_default().trim()
            )?;
        }
    }
    let mut actions = data.children("CreatureAction", &r.pk);
    actions.sort_by_key(|a| a.fields["order_in_statblock"].as_i64().unwrap_or(i64::MAX));
    for (kind, heading) in [
        ("ACTION", "Actions"),
        ("BONUS_ACTION", "Bonus Actions"),
        ("REACTION", "Reactions"),
        ("LEGENDARY_ACTION", "Legendary Actions"),
    ] {
        let group: Vec<&&Rec> = actions
            .iter()
            .filter(|a| s(a, "action_type") == Some(kind))
            .collect();
        if group.is_empty() {
            continue;
        }
        write!(t, "\n\n## {heading}")?;
        for a in group {
            let param = a.fields["uses_param"].as_i64();
            let mut usage = match (s(a, "uses_type"), param) {
                (Some("RECHARGE_ON_ROLL"), Some(6)) => " (Recharge 6)".to_owned(),
                (Some("RECHARGE_ON_ROLL"), Some(p)) => format!(" (Recharge {p}–6)"),
                (Some("PER_DAY"), Some(p)) => format!(" ({p}/Day)"),
                (Some("RECHARGE_AFTER_REST"), _) => {
                    " (Recharges after a Short or Long Rest)".to_owned()
                }
                (Some("RECHARGE"), _) => " (Recharge)".to_owned(),
                _ => String::new(),
            };
            if let Some(cost) = a.fields["legendary_action_cost"]
                .as_i64()
                .filter(|c| *c > 1)
            {
                write!(usage, " (Costs {cost} Actions)")?;
            }
            write!(
                t,
                "\n\n**{}{usage}.** {}",
                name(a)?,
                s(a, "desc").unwrap_or_default().trim()
            )?;
        }
    }
    Ok(Entry {
        book: None,
        rules: None,
        library: None,
        key: key(Kind::Monster, r),
        name: name(r)?,
        facets: Facets::Monster {
            cr,
            creature_type,
            size,
        },
        text: t,
    })
}

fn casting_time(raw: &str) -> String {
    match raw {
        "action" => "Action".into(),
        "bonus-action" => "Bonus Action".into(),
        "reaction" => "Reaction".into(),
        "round" => "1 round".into(),
        other => {
            let digits: String = other.chars().take_while(char::is_ascii_digit).collect();
            let unit = &other[digits.len()..];
            if digits.is_empty() {
                first_upper(other)
            } else {
                format!("{digits} {unit}")
            }
        }
    }
}

fn spell(data: &Data, r: &Rec) -> Result<Entry> {
    let f = &r.fields;
    let level = u8::try_from(f["level"].as_u64().context("level")?)?;
    let school = data
        .name("SpellSchool", s(r, "school").unwrap_or_default())
        .map(str::to_owned)
        .unwrap_or_else(|| title(s(r, "school").unwrap_or("Unknown")));
    let classes: Vec<String> = f["classes"]
        .as_array()
        .map(|cs| {
            cs.iter()
                .filter_map(Value::as_str)
                .map(|pk| {
                    data.name("CharacterClass", pk)
                        .map(str::to_owned)
                        .unwrap_or_else(|| title(pk.rsplit('_').next().unwrap_or(pk)))
                })
                .collect()
        })
        .unwrap_or_default();
    let ritual = f["ritual"].as_bool().unwrap_or(false);
    let concentration = f["concentration"].as_bool().unwrap_or(false);
    let mut t = if level == 0 {
        format!("*{school} Cantrip*")
    } else {
        format!("*Level {level} {school}*")
    };
    if !classes.is_empty() {
        write!(t, " ({})", classes.join(", "))?;
    }
    let mut casting = casting_time(s(r, "casting_time").unwrap_or("action"));
    if let Some(cond) = s(r, "reaction_condition") {
        write!(casting, ", {cond}")?;
    }
    if ritual {
        casting.push_str(" or Ritual");
    }
    let mut comps = Vec::new();
    for (k, c) in [("verbal", "V"), ("somatic", "S"), ("material", "M")] {
        if f[k].as_bool() == Some(true) {
            comps.push(c);
        }
    }
    let mut components = comps.join(", ");
    if let Some(m) = s(r, "material_specified") {
        write!(components, " ({m}")?;
        if f["material_consumed"].as_bool() == Some(true) {
            components.push_str(", consumed");
        }
        components.push(')');
    }
    let duration = s(r, "duration").unwrap_or("instantaneous");
    let duration = if concentration {
        format!("Concentration, up to {duration}")
    } else {
        first_upper(duration)
    };
    write!(
        t,
        "\n\n| | |\n|---|---|\n| Casting Time | {casting} |\n| Range | {} |\n| Components | {} |\n| Duration | {duration} |\n\n{}",
        s(r, "range_text").unwrap_or("Self"),
        if components.is_empty() {
            "None"
        } else {
            &components
        },
        s(r, "desc").unwrap_or_default().trim()
    )?;
    if let Some(h) = s(r, "higher_level") {
        write!(t, "\n\n**At Higher Levels.** {}", h.trim())?;
    }
    Ok(Entry {
        book: None,
        rules: None,
        library: None,
        key: key(Kind::Spell, r),
        name: name(r)?,
        facets: Facets::Spell {
            level,
            school,
            classes,
            ritual,
            concentration,
        },
        text: t,
    })
}

fn lookup_or_title(data: &Data, model: &str, pk: Option<&str>, fallback: &str) -> String {
    match pk {
        Some(pk) => data
            .name(model, pk)
            .map(str::to_owned)
            .unwrap_or_else(|| title(pk)),
        None => fallback.to_owned(),
    }
}

fn magic_item(data: &Data, r: &Rec) -> Result<Entry> {
    let rarity = lookup_or_title(data, "ItemRarity", s(r, "rarity"), "Varies");
    let category = lookup_or_title(data, "ItemCategory", s(r, "category"), "Wondrous Item");
    let attunement = r.fields["requires_attunement"].as_bool() == Some(true);
    let mut t = format!("*{category}, {rarity}");
    if attunement {
        t.push_str(" (requires attunement)");
    }
    write!(t, "*\n\n{}", s(r, "desc").unwrap_or_default().trim())?;
    Ok(Entry {
        book: None,
        rules: None,
        library: None,
        key: key(Kind::MagicItem, r),
        name: name(r)?,
        facets: Facets::MagicItem {
            rarity,
            category,
            attunement,
        },
        text: t,
    })
}

fn cost(gp: f64) -> String {
    if gp >= 1.0 || gp == 0.0 {
        format!("{} gp", trim_num(gp))
    } else if (gp * 10.0).fract().abs() < 1e-9 {
        format!("{} sp", trim_num(gp * 10.0))
    } else {
        format!("{} cp", trim_num(gp * 100.0))
    }
}

fn item(data: &Data, r: &Rec) -> Result<Entry> {
    let f = &r.fields;
    let category = lookup_or_title(data, "ItemCategory", s(r, "category"), "Adventuring Gear");
    let mut rows: Vec<(&str, String)> = Vec::new();
    if let Some(c) = num(&f["cost"]).filter(|c| *c > 0.0) {
        rows.push(("Cost", cost(c)));
    }
    if let Some(w) = num(&f["weight"]).filter(|w| *w > 0.0) {
        rows.push(("Weight", format!("{} lb.", trim_num(w))));
    }
    if let Some(wpk) = s(r, "weapon")
        && let Some(w) = data.all("Weapon").iter().find(|w| w.pk == wpk)
    {
        rows.push((
            "Damage",
            format!(
                "{} {}",
                s(w, "damage_dice").unwrap_or_default(),
                title(s(w, "damage_type").unwrap_or_default())
            ),
        ));
        let props: Vec<String> = data
            .all("WeaponPropertyAssignment")
            .iter()
            .filter(|a| a.fields["weapon"].as_str() == Some(wpk))
            .map(|a| {
                let p = a.fields["property"].as_str().unwrap_or_default();
                let n = data.name("WeaponProperty", p).unwrap_or(p).to_owned();
                match s(a, "detail") {
                    Some(d) => format!("{n} ({d})"),
                    None => n,
                }
            })
            .collect();
        if !props.is_empty() {
            rows.push(("Properties", props.join(", ")));
        }
    }
    if let Some(apk) = s(r, "armor")
        && let Some(a) = data.all("Armor").iter().find(|a| a.pk == apk)
    {
        let base = a.fields["ac_base"].as_i64().unwrap_or_default();
        let ac = match (
            a.fields["ac_add_dexmod"].as_bool() == Some(true),
            a.fields["ac_cap_dexmod"].as_i64(),
        ) {
            (true, Some(cap)) => format!("{base} + Dex modifier (max {cap})"),
            (true, None) => format!("{base} + Dex modifier"),
            (false, _) => base.to_string(),
        };
        rows.push(("Armor Class", ac));
        if let Some(st) = a.fields["strength_score_required"].as_i64() {
            rows.push(("Strength", format!("{st}+")));
        }
        if a.fields["grants_stealth_disadvantage"].as_bool() == Some(true) {
            rows.push(("Stealth", "Disadvantage".into()));
        }
    }
    let mut t = format!("*{category}*");
    if !rows.is_empty() {
        t.push_str("\n\n| | |\n|---|---|");
        for (k, v) in rows {
            write!(t, "\n| {k} | {v} |")?;
        }
    }
    if let Some(d) = s(r, "desc") {
        write!(t, "\n\n{}", d.trim())?;
    }
    Ok(Entry {
        book: None,
        rules: None,
        library: None,
        key: key(Kind::Equipment, r),
        name: name(r)?,
        facets: Facets::Equipment { category },
        text: t,
    })
}

fn benefits(data: &Data, model: &str, parent: &str) -> String {
    data.children(model, parent)
        .iter()
        .filter_map(|b| {
            let d = s(b, "desc")?.trim();
            Some(match s(b, "name") {
                Some(n) => format!("**{}.** {d}", n.trim()),
                None => format!("- {d}"),
            })
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn feat(data: &Data, r: &Rec) -> Result<Entry> {
    let category = title(s(r, "type").unwrap_or("general"));
    let mut t = format!("*{category} Feat*");
    if let Some(p) = s(r, "prerequisite") {
        write!(t, " (Prerequisite: {})", p.trim().trim_matches('*'))?;
    }
    if let Some(d) = s(r, "desc") {
        write!(t, "\n\n{}", d.trim())?;
    }
    let b = benefits(data, "FeatBenefit", &r.pk);
    if !b.is_empty() {
        write!(t, "\n\n{b}")?;
    }
    Ok(Entry {
        book: None,
        rules: None,
        library: None,
        key: key(Kind::Feat, r),
        name: name(r)?,
        facets: Facets::Feat { category },
        text: t,
    })
}

fn background(data: &Data, r: &Rec) -> Result<Entry> {
    let mut t = String::from("*Background*");
    if let Some(d) = s(r, "desc") {
        write!(t, "\n\n{}", d.trim())?;
    }
    let b = benefits(data, "BackgroundBenefit", &r.pk);
    if !b.is_empty() {
        write!(t, "\n\n{b}")?;
    }
    Ok(Entry {
        book: None,
        rules: None,
        library: None,
        key: key(Kind::Background, r),
        name: name(r)?,
        facets: Facets::Background,
        text: t,
    })
}

struct Feature<'a> {
    rec: &'a Rec,
    levels: Vec<u8>,
    column: BTreeMap<u8, String>,
}

impl Feature<'_> {
    fn first_level(&self) -> Option<u8> {
        self.levels.first().copied()
    }
}

fn features<'a>(data: &'a Data, class_pk: &str) -> Vec<Feature<'a>> {
    data.children("ClassFeature", class_pk)
        .into_iter()
        .map(|rec| {
            let items = data.children("ClassFeatureItem", &rec.pk);
            let mut column = BTreeMap::new();
            let mut levels = Vec::new();
            for i in items {
                let Some(lv) = i.fields["level"]
                    .as_u64()
                    .and_then(|l| u8::try_from(l).ok())
                else {
                    continue;
                };
                match s(i, "column_value") {
                    Some(v) => {
                        column.insert(lv, v.to_owned());
                    }
                    None => levels.push(lv),
                }
            }
            levels.sort_unstable();
            levels.dedup();
            Feature {
                rec,
                levels,
                column,
            }
        })
        .collect()
}

fn class(data: &Data, r: &Rec) -> Result<Vec<Entry>> {
    let class_name = name(r)?;
    let parent = s(r, "subclass_of");
    let base_name = match parent {
        Some(pk) => data
            .name("CharacterClass", pk)
            .map(str::to_owned)
            .unwrap_or_else(|| title(pk.rsplit('_').next().unwrap_or(pk))),
        None => class_name.clone(),
    };
    let feats = features(data, &r.pk);
    let mut t = String::new();
    match parent {
        Some(_) => write!(t, "*{base_name} subclass*")?,
        None => {
            let mut facts = Vec::new();
            if let Some(h) = s(r, "hit_dice") {
                facts.push(format!("**Hit Die** {}", h.to_lowercase()));
            }
            let saves: Vec<String> = r.fields["saving_throws"]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_str).map(title).collect())
                .unwrap_or_default();
            if !saves.is_empty() {
                facts.push(format!("**Saving Throws** {}", saves.join(", ")));
            }
            let primary: Vec<String> = r.fields["primary_abilities"]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_str).map(title).collect())
                .unwrap_or_default();
            if !primary.is_empty() {
                facts.push(format!("**Primary Ability** {}", primary.join(", ")));
            }
            if let Some(c) = s(r, "caster_type") {
                facts.push(format!("**Spellcasting** {}", title(c)));
            }
            t = if facts.is_empty() {
                "*Class*".to_owned()
            } else {
                facts.join("  \n")
            };
        }
    }
    if let Some(d) = s(r, "desc") {
        write!(t, "\n\n{}", d.trim())?;
    }
    let columns: Vec<&Feature> = feats.iter().filter(|f| !f.column.is_empty()).collect();
    let by_level: Vec<&Feature> = feats.iter().filter(|f| !f.levels.is_empty()).collect();
    if parent.is_none() && (!columns.is_empty() || !by_level.is_empty()) {
        write!(t, "\n\n## {class_name} Features\n\n| Level | Features |")?;
        for c in &columns {
            write!(t, " {} |", name(c.rec)?)?;
        }
        t.push_str("\n|---|---|");
        for _ in &columns {
            t.push_str("---|");
        }
        for lv in 1..=20u8 {
            let names: Vec<String> = by_level
                .iter()
                .filter(|f| f.levels.contains(&lv))
                .filter_map(|f| s(f.rec, "name").map(str::to_owned))
                .collect();
            write!(t, "\n| {lv} | {} |", names.join(", "))?;
            for c in &columns {
                write!(t, " {} |", c.column.get(&lv).map_or("—", String::as_str))?;
            }
        }
    }
    let mut sorted = by_level.clone();
    sorted.sort_by_key(|f| (f.first_level(), f.rec.pk.clone()));
    let mut out = Vec::new();
    for f in &sorted {
        let lv = f.first_level().unwrap_or(1);
        let fname = name(f.rec)?;
        let desc = s(f.rec, "desc").unwrap_or_default().trim().to_owned();
        write!(t, "\n\n### Level {lv}: {fname}\n\n{desc}")?;
        let header = match parent {
            Some(_) => format!("*{base_name} ({class_name}) feature, level {lv}*"),
            None => format!("*{class_name} feature, level {lv}*"),
        };
        out.push(Entry {
            book: None,
            rules: None,
            library: None,
            key: key(Kind::Feature, f.rec),
            name: fname,
            facets: Facets::Feature {
                class: base_name.clone(),
                level: lv,
            },
            text: if desc.is_empty() {
                header
            } else {
                format!("{header}\n\n{desc}")
            },
        });
    }
    out.push(Entry {
        book: None,
        rules: None,
        library: None,
        key: key(
            if parent.is_some() {
                Kind::Subclass
            } else {
                Kind::Class
            },
            r,
        ),
        name: class_name,
        facets: match parent {
            Some(_) => Facets::Subclass { class: base_name },
            None => Facets::Class,
        },
        text: t,
    });
    Ok(out)
}

fn species(data: &Data, r: &Rec) -> Result<Entry> {
    let mut t = match s(r, "subspecies_of") {
        Some(pk) => format!(
            "*Subspecies of {}*",
            lookup_or_title(data, "Species", Some(pk), "")
        ),
        None => "*Species*".to_owned(),
    };
    if let Some(d) = s(r, "desc") {
        write!(t, "\n\n{}", d.trim())?;
    }
    for tr in data.children("SpeciesTrait", &r.pk) {
        write!(
            t,
            "\n\n**{}.** {}",
            name(tr)?,
            s(tr, "desc").unwrap_or_default().trim()
        )?;
    }
    Ok(Entry {
        book: None,
        rules: None,
        library: None,
        key: key(Kind::Species, r),
        name: name(r)?,
        facets: Facets::Species,
        text: t,
    })
}

fn condition(data: &Data, r: &Rec) -> Result<Entry> {
    let describes = s(r, "describes").context("describes")?;
    let n = data
        .name("Condition", describes)
        .map(str::to_owned)
        .unwrap_or_else(|| title(describes.rsplit('_').next().unwrap_or(describes)));
    let slug = describes.rsplit('_').next().unwrap_or(describes);
    let desc = s(r, "desc").unwrap_or_default().trim();
    if desc.is_empty() {
        bail!("condition {describes} has no text");
    }
    Ok(Entry {
        book: None,
        rules: None,
        library: None,
        key: format!("condition/{slug}"),
        name: n,
        facets: Facets::Condition,
        text: desc.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(casting_time("10minutes"), "10 minutes");
        assert_eq!(casting_time("bonus-action"), "Bonus Action");
        assert_eq!(cost(0.1), "1 sp");
        assert_eq!(cost(0.02), "2 cp");
        assert_eq!(cost(25.0), "25 gp");
        assert_eq!(title("very-rare"), "Very Rare");
        assert_eq!(trim_num(1.5), "1.5");
        assert_eq!(trim_num(30.0), "30");
    }

    #[test]
    fn notices_follow_licences() {
        let meta = serde_json::json!({
            "name": "Tome of Beasts 3",
            "author": "Jane Doe",
            "publication_date": "2022-01-01T00:00:00"
        });
        let ogl = notice(&meta, "Kobold Press", &["ogl-10a".into()]);
        assert_eq!(
            ogl,
            "Tome of Beasts 3. Copyright 2022, Kobold Press. Authors: Jane Doe."
        );
        let cc = notice(&meta, "Kobold Press", &["cc-by-40".into()]);
        assert!(cc.contains("Creative Commons Attribution 4.0"), "{cc}");
    }
}
