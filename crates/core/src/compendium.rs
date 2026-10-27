use core::fmt;

use serde::{Deserialize, Serialize};

pub const BUNDLE_FORMAT: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RulesVersion {
    R2014,
    R2024,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMeta {
    pub id: String,
    pub name: String,
    pub rules: RulesVersion,
    pub upstream: String,
    pub attribution: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bundle {
    pub format: u16,
    pub source: SourceMeta,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub key: String,
    pub name: String,
    pub facets: Facets,
    pub text: String,
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct ChallengeRating(u16);

impl ChallengeRating {
    pub fn from_eighths(eighths: u16) -> Self {
        Self(eighths)
    }

    pub fn eighths(self) -> u16 {
        self.0
    }
}

impl fmt::Display for ChallengeRating {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            1 => f.write_str("1/8"),
            2 => f.write_str("1/4"),
            4 => f.write_str("1/2"),
            n if n % 8 == 0 => write!(f, "{}", n / 8),
            n => write!(f, "{n}/8"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Facets {
    Spell {
        level: u8,
        school: String,
        classes: Vec<String>,
        ritual: bool,
        concentration: bool,
    },
    Monster {
        cr: ChallengeRating,
        creature_type: String,
        size: String,
    },
    MagicItem {
        rarity: String,
        category: String,
        attunement: bool,
    },
    Equipment {
        category: String,
    },
    Class,
    Subclass {
        class: String,
    },
    Species,
    Background,
    Feat {
        category: String,
    },
    Feature {
        class: String,
        level: u8,
    },
    Trait,
    Condition,
}

impl Facets {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Spell { .. } => Kind::Spell,
            Self::Monster { .. } => Kind::Monster,
            Self::MagicItem { .. } => Kind::MagicItem,
            Self::Equipment { .. } => Kind::Equipment,
            Self::Class => Kind::Class,
            Self::Subclass { .. } => Kind::Subclass,
            Self::Species => Kind::Species,
            Self::Background => Kind::Background,
            Self::Feat { .. } => Kind::Feat,
            Self::Feature { .. } => Kind::Feature,
            Self::Trait => Kind::Trait,
            Self::Condition => Kind::Condition,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Kind {
    Spell,
    Monster,
    MagicItem,
    Equipment,
    Class,
    Subclass,
    Species,
    Background,
    Feat,
    Feature,
    Trait,
    Condition,
}

impl Kind {
    pub const ALL: [Self; 12] = [
        Self::Spell,
        Self::Monster,
        Self::MagicItem,
        Self::Equipment,
        Self::Class,
        Self::Subclass,
        Self::Species,
        Self::Background,
        Self::Feat,
        Self::Feature,
        Self::Trait,
        Self::Condition,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spell => "spell",
            Self::Monster => "monster",
            Self::MagicItem => "magic-item",
            Self::Equipment => "equipment",
            Self::Class => "class",
            Self::Subclass => "subclass",
            Self::Species => "species",
            Self::Background => "background",
            Self::Feat => "feat",
            Self::Feature => "feature",
            Self::Trait => "trait",
            Self::Condition => "condition",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_rating_display() {
        let shown: Vec<String> = [0, 1, 2, 4, 8, 24, 240]
            .map(|e| ChallengeRating::from_eighths(e).to_string())
            .into();
        assert_eq!(shown, ["0", "1/8", "1/4", "1/2", "1", "3", "30"]);
        assert!(ChallengeRating::from_eighths(2) < ChallengeRating::from_eighths(8));
    }

    #[test]
    fn kinds_round_trip_their_names() {
        for k in Kind::ALL {
            assert_eq!(Kind::parse(k.as_str()), Some(k));
        }
        assert_eq!(Kind::parse("dragon"), None);
    }
}
