use serde::{Deserialize, Serialize};

use crate::compendium::{Entry, Facets, LibraryMeta, RulesVersion};
use crate::ids::{LibraryId, UserId};
use crate::note::{NoteBody, NoteTitle};
use crate::user::ValidationError;

const MAX_FACET_CHARS: usize = 80;

const MAX_CLASSES: usize = 32;

pub const MAX_BATCH: usize = 500;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryDraft {
    pub id: LibraryId,
    pub name: NoteTitle,
    pub facets: Facets,
    pub text: NoteBody,
    pub book: Option<NoteTitle>,
    pub rules: RulesVersion,
}

impl LibraryDraft {
    pub fn check_facets(&self) -> Result<(), ValidationError> {
        let short = |s: &str| s.chars().count() <= MAX_FACET_CHARS;
        let ok = match &self.facets {
            Facets::Spell {
                level,
                school,
                classes,
                ..
            } => {
                *level <= 9
                    && short(school)
                    && classes.len() <= MAX_CLASSES
                    && classes.iter().all(|c| short(c))
            }
            Facets::Monster {
                cr,
                creature_type,
                size,
            } => cr.eighths() <= 320 && short(creature_type) && short(size),
            Facets::MagicItem {
                rarity, category, ..
            } => short(rarity) && short(category),
            Facets::Equipment { category } | Facets::Feat { category } => short(category),
            Facets::Subclass { class } => short(class),
            Facets::Feature { class, level } => short(class) && (1..=20).contains(level),
            Facets::Class
            | Facets::Species
            | Facets::Background
            | Facets::Trait
            | Facets::Condition => true,
        };
        if ok {
            Ok(())
        } else {
            Err(ValidationError::Length)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub id: LibraryId,
    pub author: UserId,
    pub name: NoteTitle,
    pub facets: Facets,
    pub text: NoteBody,
    pub book: Option<NoteTitle>,
    pub rules: RulesVersion,
    pub version: u32,
}

impl LibraryEntry {
    pub fn to_entry(&self) -> Entry {
        Entry {
            key: self.id.to_string(),
            name: self.name.as_str().to_owned(),
            facets: self.facets.clone(),
            text: self.text.as_str().to_owned(),
            book: self.book.as_ref().map(|b| b.as_str().to_owned()),
            rules: Some(self.rules),
            library: Some(LibraryMeta {
                author: self.author,
                version: self.version,
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LibraryOp {
    Save {
        draft: LibraryDraft,
        base_version: u32,
    },
    Delete {
        id: LibraryId,
    },
    SaveMany {
        drafts: Vec<LibraryDraft>,
    },
    UpsertMany {
        items: Vec<(LibraryDraft, u32)>,
    },
}

// Stored as postcard: only append variants, never change one (ADR-0003).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LibraryEvent {
    Saved { entry: LibraryEntry },
    Deleted { id: LibraryId },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportSummary {
    pub added: usize,
    pub replaced: usize,
    pub skipped: usize,
    pub not_yours: usize,
    pub invalid: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compendium::ChallengeRating;
    use uuid::Uuid;

    fn draft(facets: Facets) -> LibraryDraft {
        LibraryDraft {
            id: LibraryId(Uuid::from_bytes([1; 16])),
            name: NoteTitle::new("Ember Imp").unwrap(),
            facets,
            text: NoteBody::new("A tiny devil.".into()).unwrap(),
            book: Some(NoteTitle::new("Homebrew").unwrap()),
            rules: RulesVersion::R2024,
        }
    }

    #[test]
    fn facets_are_bounded() {
        let monster = |eighths| Facets::Monster {
            cr: ChallengeRating::from_eighths(eighths),
            creature_type: "Fiend".into(),
            size: "Tiny".into(),
        };
        assert!(draft(monster(4)).check_facets().is_ok());
        assert!(draft(monster(321)).check_facets().is_err());
        let spell = |level, school: &str| Facets::Spell {
            level,
            school: school.into(),
            classes: vec!["Wizard".into()],
            ritual: false,
            concentration: false,
        };
        assert!(draft(spell(9, "Evocation")).check_facets().is_ok());
        assert!(draft(spell(10, "Evocation")).check_facets().is_err());
        assert!(draft(spell(1, &"x".repeat(81))).check_facets().is_err());
        let feature = |level| Facets::Feature {
            class: "Wizard".into(),
            level,
        };
        assert!(draft(feature(0)).check_facets().is_err());
    }

    #[test]
    fn entries_convert_for_the_compendium() {
        let d = draft(Facets::Condition);
        let e = LibraryEntry {
            id: d.id,
            author: UserId(Uuid::from_bytes([2; 16])),
            name: d.name,
            facets: d.facets,
            text: d.text,
            book: d.book,
            rules: d.rules,
            version: 3,
        }
        .to_entry();
        assert_eq!(e.key, LibraryId(Uuid::from_bytes([1; 16])).to_string());
        assert_eq!(e.book.as_deref(), Some("Homebrew"));
        assert_eq!(e.rules, Some(RulesVersion::R2024));
    }
}
