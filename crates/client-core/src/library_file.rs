use beholder_core::compendium::{ChallengeRating, Entry, Facets, Kind, RulesVersion};
use beholder_core::{NoteBody, NoteTitle};
use serde::{Deserialize, Serialize};

pub const FORMAT: &str = "beholder-library";

pub const VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileFormat {
    Json,
    Yaml,
}

impl FileFormat {
    pub fn from_name(file_name: &str) -> Self {
        let lower = file_name.to_lowercase();
        if lower.ends_with(".yaml") || lower.ends_with(".yml") {
            Self::Yaml
        } else {
            Self::Json
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("not a Beholder library file")]
    NotLibrary,
    #[error("library file version {0} is newer than this app supports")]
    TooNew(u32),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("YAML: {0}")]
    Yaml(#[from] serde_norway::Error),
}

#[derive(Serialize, Deserialize)]
struct File {
    format: String,
    version: u32,
    entries: Vec<FileEntry>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FileEntry {
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub school: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ritual: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub concentration: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creature_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rarity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub attunement: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default)]
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parsed {
    pub name: NoteTitle,
    pub facets: Facets,
    pub text: NoteBody,
    pub book: Option<NoteTitle>,
    pub rules: RulesVersion,
}

fn rules_name(r: RulesVersion) -> &'static str {
    match r {
        RulesVersion::R2014 => "2014",
        RulesVersion::R2024 => "2024",
        RulesVersion::LevelUp => "level-up",
    }
}

fn parse_rules(s: Option<&str>) -> Result<RulesVersion, String> {
    match s.map(str::trim) {
        None | Some("2024") => Ok(RulesVersion::R2024),
        Some("2014") => Ok(RulesVersion::R2014),
        Some("level-up" | "a5e") => Ok(RulesVersion::LevelUp),
        Some(other) => Err(format!("unknown rules version \"{other}\"")),
    }
}

fn parse_cr(s: &str) -> Option<ChallengeRating> {
    let s = s.trim();
    let eighths = match s.split_once('/') {
        Some((n, d)) => {
            let (n, d): (u16, u16) = (n.trim().parse().ok()?, d.trim().parse().ok()?);
            (d != 0 && 8 % d == 0).then(|| n * (8 / d))?
        }
        None => {
            let f: f64 = s.parse().ok()?;
            let e = (f * 8.0).round();
            if !(0.0..=320.0).contains(&e) {
                return None;
            }
            e as u16
        }
    };
    Some(ChallengeRating::from_eighths(eighths))
}

fn entry_to_file(e: &Entry) -> FileEntry {
    let mut f = FileEntry {
        kind: e.facets.kind().as_str().to_owned(),
        name: e.name.clone(),
        book: e.book.clone(),
        rules: e.rules.map(|r| rules_name(r).to_owned()),
        text: e.text.clone(),
        ..FileEntry::default()
    };
    match &e.facets {
        Facets::Spell {
            level,
            school,
            classes,
            ritual,
            concentration,
        } => {
            f.level = Some(*level);
            f.school = Some(school.clone());
            f.classes = classes.clone();
            f.ritual = *ritual;
            f.concentration = *concentration;
        }
        Facets::Monster {
            cr,
            creature_type,
            size,
        } => {
            f.cr = Some(cr.to_string());
            f.creature_type = Some(creature_type.clone());
            f.size = Some(size.clone());
        }
        Facets::MagicItem {
            rarity,
            category,
            attunement,
        } => {
            f.rarity = Some(rarity.clone());
            f.category = Some(category.clone());
            f.attunement = *attunement;
        }
        Facets::Equipment { category } | Facets::Feat { category } => {
            f.category = Some(category.clone());
        }
        Facets::Subclass { class } => f.class = Some(class.clone()),
        Facets::Feature { class, level } => {
            f.class = Some(class.clone());
            f.level = Some(*level);
        }
        Facets::Class
        | Facets::Species
        | Facets::Background
        | Facets::Trait
        | Facets::Condition => {}
    }
    f
}

pub fn parse_entry(f: &FileEntry) -> Result<Parsed, String> {
    let req = |v: &Option<String>, field: &str| {
        v.clone()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| format!("missing {field}"))
    };
    let kind = Kind::parse(f.kind.trim()).ok_or_else(|| format!("unknown kind \"{}\"", f.kind))?;
    let facets = match kind {
        Kind::Spell => Facets::Spell {
            level: f.level.unwrap_or(0),
            school: req(&f.school, "school")?,
            classes: f.classes.clone(),
            ritual: f.ritual,
            concentration: f.concentration,
        },
        Kind::Monster => Facets::Monster {
            cr: f
                .cr
                .as_deref()
                .and_then(parse_cr)
                .ok_or_else(|| "missing or invalid cr".to_owned())?,
            creature_type: req(&f.creature_type, "creature_type")?,
            size: req(&f.size, "size")?,
        },
        Kind::MagicItem => Facets::MagicItem {
            rarity: req(&f.rarity, "rarity")?,
            category: req(&f.category, "category")?,
            attunement: f.attunement,
        },
        Kind::Equipment => Facets::Equipment {
            category: req(&f.category, "category")?,
        },
        Kind::Feat => Facets::Feat {
            category: req(&f.category, "category")?,
        },
        Kind::Subclass => Facets::Subclass {
            class: req(&f.class, "class")?,
        },
        Kind::Feature => Facets::Feature {
            class: req(&f.class, "class")?,
            level: f.level.ok_or("missing level")?,
        },
        Kind::Class => Facets::Class,
        Kind::Species => Facets::Species,
        Kind::Background => Facets::Background,
        Kind::Trait => Facets::Trait,
        Kind::Condition => Facets::Condition,
    };
    Ok(Parsed {
        name: NoteTitle::new(&f.name).map_err(|e| format!("name: {e}"))?,
        facets,
        text: NoteBody::new(f.text.clone()).map_err(|e| format!("text: {e}"))?,
        book: f
            .book
            .as_deref()
            .filter(|b| !b.trim().is_empty())
            .map(NoteTitle::new)
            .transpose()
            .map_err(|e| format!("book: {e}"))?,
        rules: parse_rules(f.rules.as_deref())?,
    })
}

pub fn export(entries: &[Entry], format: FileFormat) -> Result<String, FileError> {
    let file = File {
        format: FORMAT.to_owned(),
        version: VERSION,
        entries: entries.iter().map(entry_to_file).collect(),
    };
    Ok(match format {
        FileFormat::Json => serde_json::to_string_pretty(&file)?,
        FileFormat::Yaml => serde_norway::to_string(&file)?,
    })
}

pub fn read(text: &str, format: FileFormat) -> Result<Vec<FileEntry>, FileError> {
    let file: File = match format {
        FileFormat::Json => serde_json::from_str(text)?,
        FileFormat::Yaml => serde_norway::from_str(text)?,
    };
    if file.format != FORMAT {
        return Err(FileError::NotLibrary);
    }
    if file.version > VERSION {
        return Err(FileError::TooNew(file.version));
    }
    Ok(file.entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spell() -> Entry {
        Entry {
            key: "x".into(),
            name: "Ember Lance".into(),
            facets: Facets::Spell {
                level: 2,
                school: "Evocation".into(),
                classes: vec!["Wizard".into()],
                ritual: false,
                concentration: true,
            },
            text: "A lance of *fire*.".into(),
            book: Some("Homebrew".into()),
            rules: Some(RulesVersion::R2024),
            library: None,
        }
    }

    #[test]
    fn round_trips_through_json_and_yaml() {
        for fmt in [FileFormat::Json, FileFormat::Yaml] {
            let text = export(&[spell()], fmt).unwrap();
            let back = read(&text, fmt).unwrap();
            let p = parse_entry(&back[0]).unwrap();
            assert_eq!(p.name.as_str(), "Ember Lance");
            assert_eq!(p.facets, spell().facets);
            assert_eq!(
                p.book.map(|b| b.as_str().to_owned()).as_deref(),
                Some("Homebrew")
            );
            assert_eq!(p.rules, RulesVersion::R2024);
            assert_eq!(p.text.as_str(), "A lance of *fire*.");
        }
    }

    #[test]
    fn hand_written_yaml_is_accepted() {
        let text = "format: beholder-library\nversion: 1\nentries:\n  - kind: monster\n    name: Ember Imp\n    rules: \"2014\"\n    cr: 1/4\n    creature_type: Fiend\n    size: Tiny\n    text: |\n      **AC** 13\n";
        let entries = read(text, FileFormat::Yaml).unwrap();
        let p = parse_entry(&entries[0]).unwrap();
        assert_eq!(
            p.facets,
            Facets::Monster {
                cr: ChallengeRating::from_eighths(2),
                creature_type: "Fiend".into(),
                size: "Tiny".into()
            }
        );
        assert_eq!(p.rules, RulesVersion::R2014);
    }

    #[test]
    fn bad_entries_explain_themselves() {
        let mut f = entry_to_file(&spell());
        f.school = None;
        assert_eq!(parse_entry(&f).unwrap_err(), "missing school");
        f.kind = "vehicle".into();
        assert!(parse_entry(&f).unwrap_err().contains("unknown kind"));
        let mut m = FileEntry {
            kind: "monster".into(),
            name: "X".into(),
            cr: Some("1/3".into()),
            creature_type: Some("Beast".into()),
            size: Some("Small".into()),
            ..FileEntry::default()
        };
        assert!(parse_entry(&m).is_err(), "1/3 isn't a challenge rating");
        m.cr = Some("0.5".into());
        assert!(parse_entry(&m).is_ok());
        assert!(matches!(
            read(
                "{\"format\":\"other\",\"version\":1,\"entries\":[]}",
                FileFormat::Json
            ),
            Err(FileError::NotLibrary)
        ));
        assert!(matches!(
            read(
                "{\"format\":\"beholder-library\",\"version\":9,\"entries\":[]}",
                FileFormat::Json
            ),
            Err(FileError::TooNew(9))
        ));
    }
}
