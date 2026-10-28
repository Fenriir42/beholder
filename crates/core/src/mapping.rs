use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::compendium::{Kind, RulesVersion};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Field {
    Level,
    School,
    Classes,
    Ritual,
    Concentration,
    Cr,
    CreatureType,
    Size,
    Rarity,
    Category,
    Attunement,
    Class,
}

impl Field {
    pub const ALL: [Self; 12] = [
        Self::Level,
        Self::School,
        Self::Classes,
        Self::Ritual,
        Self::Concentration,
        Self::Cr,
        Self::CreatureType,
        Self::Size,
        Self::Rarity,
        Self::Category,
        Self::Attunement,
        Self::Class,
    ];
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub entries: Vec<String>,
    pub name_key: String,
    pub text_keys: Vec<String>,
    pub kind_key: Option<String>,
    pub kinds: BTreeMap<String, Kind>,
    pub default_kind: Kind,
    pub book_key: Option<String>,
    pub book: Option<String>,
    pub rules: RulesVersion,
    pub rules_from_book: bool,
    pub fields: BTreeMap<Field, String>,
}
