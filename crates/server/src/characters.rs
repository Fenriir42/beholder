use beholder_core::character::{Character, CharacterOp};
use beholder_core::{CampaignEvent, CampaignId, CharacterId};
use beholder_protocol::RejectReason;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::db::DbError;
use crate::permissions::{self, Viewer};

pub(crate) fn decide(
    tx: &Transaction<'_>,
    viewer: Viewer,
    campaign_id: CampaignId,
    op: &CharacterOp,
) -> Result<Result<Vec<CampaignEvent>, RejectReason>, DbError> {
    let characters = campaign_characters(tx, campaign_id)?;
    match op {
        CharacterOp::Create { id, .. } => {
            if exists(tx, *id)? {
                return Ok(Err(RejectReason::IdConflict));
            }
        }
        CharacterOp::Change { id, .. }
        | CharacterOp::SetActive { id }
        | CharacterOp::Delete { id } => {
            let Some(c) = characters.iter().find(|c| c.id == *id) else {
                return Ok(Err(RejectReason::NotFound));
            };
            if !permissions::can_edit_character(viewer, c) {
                return Ok(Err(RejectReason::Forbidden));
            }
        }
    }
    Ok(op
        .events(viewer.user, &characters)
        .map_err(RejectReason::Invalid))
}

fn exists(conn: &Connection, id: CharacterId) -> Result<bool, DbError> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM characters WHERE id = ?1",
            [id.as_bytes()],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

pub(crate) fn project(
    tx: &Transaction<'_>,
    campaign_id: CampaignId,
    event: &CampaignEvent,
) -> Result<(), DbError> {
    match event {
        CampaignEvent::CharacterSaved { character } => {
            tx.execute(
                "INSERT INTO characters (id, campaign_id, owner, body) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (id) DO UPDATE SET body = excluded.body",
                params![
                    character.id.as_bytes(),
                    campaign_id.as_bytes(),
                    character.owner.as_bytes(),
                    postcard::to_stdvec(&**character)?
                ],
            )?;
        }
        CampaignEvent::CharacterDeleted { id } => {
            tx.execute(
                "UPDATE characters SET deleted = 1 WHERE id = ?1",
                [id.as_bytes()],
            )?;
        }
        _ => {}
    }
    Ok(())
}

// Characters stored before PROTOCOL_VERSION 6 (no rest, proficiency or concentration fields).
#[derive(serde::Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
struct LegacyCounter {
    name: String,
    current: u16,
    max: u16,
}

#[derive(serde::Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
struct LegacyItem {
    item: beholder_core::character::Link,
    quantity: u32,
    equipped: bool,
    note: String,
}

#[derive(serde::Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
struct Legacy {
    id: CharacterId,
    owner: beholder_core::UserId,
    name: String,
    rules: beholder_core::compendium::RulesVersion,
    species: Option<beholder_core::character::Link>,
    background: Option<beholder_core::character::Link>,
    classes: Vec<beholder_core::character::ClassLevel>,
    feats: Vec<beholder_core::character::Link>,
    features: Vec<beholder_core::character::Link>,
    abilities: [u8; 6],
    saves: [bool; 6],
    skills: [beholder_core::rules::Proficiency; 18],
    jack_of_all_trades: bool,
    armor: beholder_core::rules::ArmorFormula,
    shield: bool,
    ac_bonus: i8,
    initiative_bonus: i8,
    speed: u16,
    hp: beholder_core::character::HitPoints,
    hit_dice_spent: u8,
    death_saves: (u8, u8),
    conditions: Vec<String>,
    exhaustion: u8,
    counters: Vec<LegacyCounter>,
    slots_spent: [u8; 9],
    pact_spent: u8,
    spells: Vec<beholder_core::character::KnownSpell>,
    inventory: Vec<LegacyItem>,
    currency: [u32; 5],
    notes: String,
    active: bool,
}

impl From<Legacy> for Character {
    fn from(l: Legacy) -> Self {
        use beholder_core::character::{Counter, Item, Reset};
        Self {
            id: l.id,
            owner: l.owner,
            name: l.name,
            rules: l.rules,
            species: l.species,
            background: l.background,
            classes: l.classes,
            feats: l.feats,
            features: l.features,
            abilities: l.abilities,
            saves: l.saves,
            skills: l.skills,
            jack_of_all_trades: l.jack_of_all_trades,
            armor: l.armor,
            shield: l.shield,
            ac_bonus: l.ac_bonus,
            initiative_bonus: l.initiative_bonus,
            speed: l.speed,
            hp: l.hp,
            hit_dice_spent: l.hit_dice_spent,
            death_saves: l.death_saves,
            conditions: l.conditions,
            exhaustion: l.exhaustion,
            counters: l
                .counters
                .into_iter()
                .map(|c| Counter {
                    name: c.name,
                    current: c.current,
                    max: c.max,
                    reset: Reset::LongRest,
                })
                .collect(),
            slots_spent: l.slots_spent,
            pact_spent: l.pact_spent,
            spells: l.spells,
            inventory: l
                .inventory
                .into_iter()
                .map(|i| Item {
                    item: i.item,
                    quantity: i.quantity,
                    equipped: i.equipped,
                    note: i.note,
                    proficient: true,
                })
                .collect(),
            currency: l.currency,
            notes: l.notes,
            active: l.active,
            concentration: None,
        }
    }
}

fn decode(body: &[u8]) -> Result<Character, DbError> {
    match postcard::from_bytes::<Character>(body) {
        Ok(c) => Ok(c),
        Err(_) => Ok(postcard::from_bytes::<Legacy>(body)?.into()),
    }
}

pub fn campaign_characters(
    conn: &Connection,
    campaign_id: CampaignId,
) -> Result<Vec<Character>, DbError> {
    let mut stmt = conn.prepare_cached(
        "SELECT body FROM characters WHERE campaign_id = ?1 AND deleted = 0 ORDER BY id",
    )?;
    stmt.query_map([campaign_id.as_bytes()], |r| r.get::<_, Vec<u8>>(0))?
        .map(|row| decode(&row?))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use beholder_core::character::{Link, Reset};

    #[test]
    fn characters_stored_before_version_6_still_load() {
        let c = Character::new(
            CharacterId(uuid::Uuid::from_bytes([1; 16])),
            beholder_core::UserId(uuid::Uuid::from_bytes([2; 16])),
            "Misaka".into(),
            beholder_core::compendium::RulesVersion::R2014,
        );
        let old = Legacy {
            id: c.id,
            owner: c.owner,
            name: c.name.clone(),
            rules: c.rules,
            species: None,
            background: None,
            classes: Vec::new(),
            feats: Vec::new(),
            features: Vec::new(),
            abilities: c.abilities,
            saves: c.saves,
            skills: c.skills,
            jack_of_all_trades: false,
            armor: c.armor,
            shield: false,
            ac_bonus: 0,
            initiative_bonus: 0,
            speed: 30,
            hp: c.hp,
            hit_dice_spent: 0,
            death_saves: (0, 0),
            conditions: Vec::new(),
            exhaustion: 0,
            counters: vec![LegacyCounter {
                name: "Sorcery points".into(),
                current: 2,
                max: 5,
            }],
            slots_spent: [0; 9],
            pact_spent: 0,
            spells: Vec::new(),
            inventory: vec![LegacyItem {
                item: Link {
                    name: "Rapier".into(),
                    ..Link::default()
                },
                quantity: 1,
                equipped: true,
                note: String::new(),
            }],
            currency: [0; 5],
            notes: "kept".into(),
            active: true,
        };
        let loaded = decode(&postcard::to_stdvec(&old).unwrap()).unwrap();
        assert_eq!(loaded.notes, "kept");
        assert_eq!(loaded.counters[0].reset, Reset::LongRest);
        assert!(loaded.inventory[0].proficient && loaded.active);
        assert_eq!(
            decode(&postcard::to_stdvec(&loaded).unwrap()).unwrap(),
            loaded
        );
    }
}
