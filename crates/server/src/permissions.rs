use std::collections::HashMap;

use beholder_core::character::{Character, HitPoints};
use beholder_core::combat::{Encounter, Shape, ShapeKind, Source, health};
use beholder_core::dice::{RollRecord, Visibility};
use beholder_core::{
    Audience, Campaign, CampaignEvent, CampaignId, CampaignOp, CampaignSettings, CombatVisibility,
    EncounterId, Note, NoteId, Role, Seq, ShapeId, UserId,
};
use beholder_protocol::{EventEnvelope, ServerMsg};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Viewer {
    pub user: UserId,
    pub role: Role,
}

pub fn can_apply(role: Role, op: &CampaignOp) -> bool {
    match op {
        CampaignOp::Rename { .. }
        | CampaignOp::SetCover { .. }
        | CampaignOp::RegenerateInvite
        | CampaignOp::UpdateSettings(_)
        | CampaignOp::SetSources { .. }
        | CampaignOp::HideEntry { .. } => role == Role::Dm,
    }
}

fn reads(viewer: Viewer, settings: &CampaignSettings, author: UserId, audience: &Audience) -> bool {
    author == viewer.user
        || audience.includes(viewer.user)
        || (*audience == Audience::Private
            && viewer.role == Role::Dm
            && settings.dm_can_read_private_notes)
}

pub fn can_read_note(viewer: Viewer, settings: &CampaignSettings, note: &Note) -> bool {
    reads(viewer, settings, note.author, &note.audience)
}

pub fn can_edit_note(viewer: Viewer, note: &Note) -> bool {
    note.author == viewer.user || note.audience.includes(viewer.user)
}

pub fn can_manage_note(viewer: Viewer, note: &Note) -> bool {
    note.author == viewer.user
}

pub fn can_edit_character(viewer: Viewer, character: &Character) -> bool {
    character.owner == viewer.user || viewer.role == Role::Dm
}

pub fn can_see_roll(viewer: Viewer, roll: &RollRecord) -> bool {
    roll.visibility == Visibility::Public || roll.roller == viewer.user || viewer.role == Role::Dm
}

pub struct Scope<'a> {
    pub settings: CampaignSettings,
    pub notes: &'a HashMap<NoteId, Note>,
    pub encounters: &'a HashMap<EncounterId, Encounter>,
}

pub fn can_move_shape(viewer: Viewer, e: &Encounter, shape: ShapeId) -> bool {
    viewer.role == Role::Dm
        || (e.active
            && !e.locked
            && e.shapes
                .iter()
                .any(|s| s.id == shape && s.owner == Some(viewer.user)))
}

pub fn acts_for(
    viewer: Viewer,
    e: &Encounter,
    combatant: beholder_core::CombatantId,
    characters: &[Character],
) -> bool {
    viewer.role == Role::Dm
        || e.combatants
            .iter()
            .find(|c| c.id == combatant)
            .is_some_and(|c| match c.source {
                Source::Character(id) => characters
                    .iter()
                    .any(|ch| ch.id == id && ch.owner == viewer.user),
                _ => false,
            })
}

pub fn can_preview_as(viewer: Viewer) -> bool {
    viewer.role == Role::Dm
}

pub const MAX_PLAYER_TEMPLATES: usize = 10;

fn player_template(viewer: Viewer, s: &Shape) -> bool {
    matches!(s.kind, ShapeKind::Template(_)) && s.owner == Some(viewer.user)
}

pub fn can_put_shape(viewer: Viewer, e: &Encounter, shape: &Shape) -> bool {
    if viewer.role == Role::Dm {
        return true;
    }
    let existing = e.shapes.iter().find(|s| s.id == shape.id);
    let room = existing.is_some()
        || e.shapes
            .iter()
            .filter(|s| player_template(viewer, s))
            .count()
            < MAX_PLAYER_TEMPLATES;
    e.active
        && !e.locked
        && player_template(viewer, shape)
        && !shape.hidden
        && shape.combatant.is_none()
        && existing.is_none_or(|s| player_template(viewer, s))
        && room
}

pub fn can_remove_shape(viewer: Viewer, e: &Encounter, shape: ShapeId) -> bool {
    viewer.role == Role::Dm
        || (e.active
            && !e.locked
            && e.shapes
                .iter()
                .any(|s| s.id == shape && player_template(viewer, s)))
}

pub fn can_preview(viewer: Viewer, e: &Encounter, shape: ShapeId) -> bool {
    e.live_drag && can_move_shape(viewer, e, shape)
}

pub fn sees_shape(viewer: Viewer, e: &Encounter, shape: ShapeId) -> bool {
    viewer.role == Role::Dm || (e.active && e.shapes.iter().any(|s| s.id == shape && !s.hidden))
}

pub fn encounter_for(
    viewer: Viewer,
    settings: &CampaignSettings,
    e: &Encounter,
) -> Option<Encounter> {
    if viewer.role == Role::Dm {
        return Some(e.clone());
    }
    if !e.active {
        return None;
    }
    let mut out = e.clone();
    out.shapes.retain(|s| !s.hidden);
    out.combatants.retain(|c| !c.hidden);
    if out
        .turn
        .is_some_and(|t| !out.combatants.iter().any(|c| c.id == t))
    {
        out.turn = None;
    }
    match settings.combat_visibility {
        CombatVisibility::Exact => {}
        CombatVisibility::Vague => {
            for c in &mut out.combatants {
                if !matches!(c.source, Source::Character(_)) {
                    c.health = Some(health(c.hp));
                    c.hp = HitPoints::default();
                    c.ac = None;
                }
            }
        }
        CombatVisibility::Hidden => {
            out.combatants.clear();
            out.round = 0;
            out.turn = None;
        }
    }
    Some(out)
}

pub fn event_for(
    viewer: Viewer,
    scope: &Scope<'_>,
    event: &CampaignEvent,
) -> Option<CampaignEvent> {
    let settings = &scope.settings;
    match event {
        CampaignEvent::InviteRegenerated { .. } => (viewer.role == Role::Dm).then(|| event.clone()),
        CampaignEvent::Created { .. }
        | CampaignEvent::Renamed { .. }
        | CampaignEvent::CoverChanged { .. }
        | CampaignEvent::MemberJoined { .. }
        | CampaignEvent::SettingsChanged { .. }
        | CampaignEvent::SourcesChanged { .. }
        | CampaignEvent::EntryHidden { .. }
        | CampaignEvent::CharacterDeleted { .. }
        | CampaignEvent::PartyMemberSaved { .. } => Some(event.clone()),
        CampaignEvent::Rolled { roll } => can_see_roll(viewer, roll).then(|| event.clone()),
        CampaignEvent::EncounterSaved { encounter } => {
            Some(match encounter_for(viewer, settings, encounter) {
                Some(e) => CampaignEvent::EncounterSaved {
                    encounter: Box::new(e),
                },
                None => CampaignEvent::EncounterDeleted { id: encounter.id },
            })
        }
        CampaignEvent::EncounterDeleted { .. } => Some(event.clone()),
        CampaignEvent::ShapeMoved {
            encounter, shape, ..
        } => {
            let e = scope.encounters.get(encounter)?;
            sees_shape(viewer, e, *shape).then(|| event.clone())
        }
        CampaignEvent::CharacterSaved { character } => {
            if can_edit_character(viewer, character) {
                Some(event.clone())
            } else {
                Some(CampaignEvent::PartyMemberSaved {
                    summary: character.summary(),
                })
            }
        }
        CampaignEvent::NoteCreated { note } => {
            can_read_note(viewer, settings, note).then(|| event.clone())
        }
        CampaignEvent::NoteEdited { id, .. } | CampaignEvent::NoteDeleted { id } => {
            let note = scope.notes.get(id)?;
            can_read_note(viewer, settings, note).then(|| event.clone())
        }
        CampaignEvent::NoteAudienceChanged {
            id,
            audience,
            previous,
        } => {
            let note = scope.notes.get(id)?;
            let before = reads(viewer, settings, note.author, previous);
            let after = reads(viewer, settings, note.author, audience);
            match (before, after) {
                (true, true) => Some(event.clone()),
                (false, true) => Some(CampaignEvent::NoteCreated {
                    note: Note {
                        audience: audience.clone(),
                        ..note.clone()
                    },
                }),
                (true, false) => Some(CampaignEvent::NoteDeleted { id: *id }),
                (false, false) => None,
            }
        }
    }
}

pub fn campaign_for(viewer: Viewer, campaign: &Campaign) -> Campaign {
    Campaign {
        id: campaign.id,
        title: campaign.title.clone(),
        cover: campaign.cover,
        settings: campaign.settings,
        invite_code: campaign.invite_code.filter(|_| viewer.role == Role::Dm),
        members: campaign.members.clone(),
        notes: campaign
            .notes
            .iter()
            .filter(|n| can_read_note(viewer, &campaign.settings, n))
            .cloned()
            .collect(),
        sources: campaign.sources.clone(),
        hidden: campaign.hidden.clone(),
        characters: campaign
            .characters
            .iter()
            .filter(|c| can_edit_character(viewer, c))
            .cloned()
            .collect(),
        party: campaign
            .characters
            .iter()
            .filter(|c| !can_edit_character(viewer, c))
            .map(Character::summary)
            .collect(),
        rolls: campaign
            .rolls
            .iter()
            .filter(|r| can_see_roll(viewer, r))
            .cloned()
            .collect(),
        encounters: campaign
            .encounters
            .iter()
            .filter_map(|e| encounter_for(viewer, &campaign.settings, e))
            .collect(),
    }
}

pub fn can_edit_library(actor: UserId, author: UserId, actor_is_dm_somewhere: bool) -> bool {
    actor == author || actor_is_dm_somewhere
}

pub fn can_undo_import(actor: UserId, importer: UserId, actor_is_dm_somewhere: bool) -> bool {
    actor == importer || actor_is_dm_somewhere
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outgoing(ServerMsg);

impl Outgoing {
    pub fn events(
        viewer: Viewer,
        scope: &Scope<'_>,
        campaign_id: CampaignId,
        events: &[EventEnvelope],
    ) -> Option<Self> {
        let events: Vec<_> = events
            .iter()
            .filter_map(|e| {
                event_for(viewer, scope, &e.event).map(|event| EventEnvelope {
                    seq: e.seq,
                    ts: e.ts,
                    actor: e.actor,
                    event,
                })
            })
            .collect();
        (!events.is_empty()).then_some(Self(ServerMsg::Events {
            campaign_id,
            events,
        }))
    }

    pub fn snapshot(viewer: Viewer, seq: Seq, campaign: &Campaign) -> Self {
        Self(ServerMsg::Snapshot {
            seq,
            campaign: campaign_for(viewer, campaign),
        })
    }

    pub fn preview(
        viewer: Viewer,
        e: &Encounter,
        preview: beholder_protocol::Preview,
    ) -> Option<Self> {
        sees_shape(viewer, e, preview.shape).then_some(Self(ServerMsg::Preview(preview)))
    }

    pub fn control(msg: ServerMsg) -> Option<Self> {
        match msg {
            ServerMsg::Events { .. } | ServerMsg::Snapshot { .. } | ServerMsg::Preview(_) => None,
            ServerMsg::HelloOk(_)
            | ServerMsg::HelloErr(_)
            | ServerMsg::Ack { .. }
            | ServerMsg::Reject { .. }
            | ServerMsg::CampaignRemoved { .. }
            | ServerMsg::Presence { .. }
            | ServerMsg::PresenceChanged { .. }
            | ServerMsg::Pong { .. }
            | ServerMsg::LibraryChanged { .. } => Some(Self(msg)),
        }
    }

    pub fn presence(campaign_id: CampaignId, online: Vec<UserId>) -> Self {
        Self(ServerMsg::Presence {
            campaign_id,
            online,
        })
    }

    pub fn presence_changed(campaign_id: CampaignId, user_id: UserId, online: bool) -> Self {
        Self(ServerMsg::PresenceChanged {
            campaign_id,
            user_id,
            online,
        })
    }

    pub fn msg(&self) -> &ServerMsg {
        &self.0
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use beholder_core::{
        CampaignSettings, CampaignTitle, CombatVisibility, DisplayName, ImageHash, InviteCode,
        Member, NoteBody, NoteTitle, UnixMillis, UserId,
    };
    use beholder_protocol::HelloError;
    use uuid::Uuid;

    const ROLES: [Role; 2] = [Role::Dm, Role::Player];

    fn viewer(role: Role) -> Viewer {
        let n = if role == Role::Dm { 1 } else { 2 };
        Viewer {
            user: member(n, role).user_id,
            role,
        }
    }

    fn no_notes() -> HashMap<NoteId, Note> {
        HashMap::new()
    }

    fn scope(notes: &HashMap<NoteId, Note>, dm_reads: bool) -> Scope<'_> {
        Scope {
            settings: CampaignSettings {
                dm_can_read_private_notes: dm_reads,
                combat_visibility: CombatVisibility::Exact,
            },
            notes,
            encounters: Box::leak(Box::new(HashMap::new())),
        }
    }

    const DM: u8 = 1;
    const P1: u8 = 2;
    const P2: u8 = 3;

    fn uid(n: u8) -> UserId {
        UserId(Uuid::from_bytes([n; 16]))
    }

    fn v(n: u8) -> Viewer {
        Viewer {
            user: uid(n),
            role: if n == DM { Role::Dm } else { Role::Player },
        }
    }

    fn note(author: u8, audience: Audience) -> Note {
        let tag = match &audience {
            Audience::Private => 0,
            Audience::Party => 1,
            Audience::Users(u) => 2 + u.first().map_or(0, |x| x.as_bytes()[0]),
        };
        let mut id = [0; 16];
        id[0] = author;
        id[1] = tag;
        Note {
            id: NoteId(Uuid::from_bytes(id)),
            author: uid(author),
            title: NoteTitle::new("secret").unwrap(),
            body: NoteBody::new("the vampire is the priest".into()).unwrap(),
            audience,
            version: 1,
            conflict_of: None,
        }
    }

    // (note, dm_can_read_private_notes, [DM sees, P1 sees, P2 sees])
    fn read_matrix() -> Vec<(Note, bool, [bool; 3])> {
        let mut rows = Vec::new();
        for dm_reads in [false, true] {
            rows.extend([
                (note(DM, Audience::Private), dm_reads, [true, false, false]),
                (note(DM, Audience::Party), dm_reads, [true, true, true]),
                (
                    note(DM, Audience::Users(vec![uid(P2)])),
                    dm_reads,
                    [true, false, true],
                ),
                (
                    note(P1, Audience::Private),
                    dm_reads,
                    [dm_reads, true, false],
                ),
                (note(P1, Audience::Party), dm_reads, [true, true, true]),
                (
                    note(P1, Audience::Users(vec![uid(P2)])),
                    dm_reads,
                    [false, true, true],
                ),
                (
                    note(P1, Audience::Users(vec![uid(DM)])),
                    dm_reads,
                    [true, true, false],
                ),
            ]);
        }
        rows
    }

    #[test]
    fn note_read_matrix() {
        for (n, dm_reads, expected) in read_matrix() {
            let notes = no_notes();
            let sc = scope(&notes, dm_reads);
            for (who, want) in [DM, P1, P2].into_iter().zip(expected) {
                assert_eq!(
                    can_read_note(v(who), &sc.settings, &n),
                    want,
                    "viewer {who} dm_reads {dm_reads} {:?} by {}",
                    n.audience,
                    n.author
                );
                let created = CampaignEvent::NoteCreated { note: n.clone() };
                assert_eq!(event_for(v(who), &sc, &created).is_some(), want);
                let index = HashMap::from([(n.id, n.clone())]);
                let sc = scope(&index, dm_reads);
                for e in [
                    CampaignEvent::NoteEdited {
                        id: n.id,
                        title: n.title.clone(),
                        body: n.body.clone(),
                        version: 2,
                    },
                    CampaignEvent::NoteDeleted { id: n.id },
                ] {
                    assert_eq!(event_for(v(who), &sc, &e).is_some(), want, "{e:?}");
                }
            }
        }
    }
    #[test]
    fn combat_matrix() {
        use beholder_core::character::HitPoints;
        use beholder_core::combat::{Colour, Combatant, Health, Shape, ShapeKind, Source};
        let cid = |n: u8| beholder_core::CombatantId(Uuid::from_bytes([n; 16]));
        let sid = |n: u8| ShapeId(Uuid::from_bytes([n; 16]));
        let fighter = |n: u8, source: Source, hidden: bool| Combatant {
            id: cid(n),
            name: format!("C{n}"),
            source,
            initiative: Some(10),
            hp: HitPoints {
                current: 4,
                max: 20,
                temp: 0,
            },
            ac: Some(13),
            conditions: Vec::new(),
            concentration: false,
            hidden,
            health: None,
            speed: None,
            used: beholder_core::combat::TurnUse::default(),
        };
        let shape = |n: u8, hidden: bool, owner: Option<UserId>| Shape {
            id: sid(n),
            kind: ShapeKind::Token,
            x: 1,
            y: 1,
            w: 1,
            h: 1,
            label: String::new(),
            colour: Colour::Palette(0),
            hidden,
            combatant: None,
            owner,
            note: String::new(),
        };
        let mut e = Encounter::new(EncounterId(Uuid::from_bytes([1; 16])), "Fight".into());
        e.active = true;
        e.combatants = vec![
            fighter(
                1,
                Source::Character(beholder_core::CharacterId(Uuid::from_bytes([7; 16]))),
                false,
            ),
            fighter(2, Source::Custom, false),
            fighter(3, Source::Custom, true),
        ];
        e.shapes = vec![shape(1, false, Some(uid(P1))), shape(2, true, None)];
        let settings = |v| CampaignSettings {
            dm_can_read_private_notes: false,
            combat_visibility: v,
        };
        let dm_view = encounter_for(v(DM), &settings(CombatVisibility::Vague), &e).unwrap();
        assert_eq!(
            (dm_view.combatants.len(), dm_view.shapes.len()),
            (3, 2),
            "the DM sees everything"
        );
        let exact = encounter_for(v(P2), &settings(CombatVisibility::Exact), &e).unwrap();
        assert_eq!(
            (exact.combatants.len(), exact.shapes.len()),
            (2, 1),
            "hidden things never reach players"
        );
        assert_eq!(exact.combatants[1].hp.current, 4);
        let vague = encounter_for(v(P2), &settings(CombatVisibility::Vague), &e).unwrap();
        assert_eq!(vague.combatants[0].hp.current, 4, "PC HP is always exact");
        assert_eq!(
            (
                vague.combatants[1].hp,
                vague.combatants[1].ac,
                vague.combatants[1].health
            ),
            (HitPoints::default(), None, Some(Health::NearDeath)),
            "monster HP becomes a band"
        );
        let hidden = encounter_for(v(P2), &settings(CombatVisibility::Hidden), &e).unwrap();
        assert!(
            hidden.combatants.is_empty() && hidden.shapes.len() == 1,
            "map only"
        );
        let mut idle = e.clone();
        idle.active = false;
        assert!(
            encounter_for(v(P1), &settings(CombatVisibility::Exact), &idle).is_none(),
            "prepared encounters stay with the DM"
        );
        // (viewer, may move the P1 token, may move the hidden token)
        for (who, own, other) in [(DM, true, true), (P1, true, false), (P2, false, false)] {
            assert_eq!(can_move_shape(v(who), &e, sid(1)), own, "{who}");
            assert_eq!(can_move_shape(v(who), &e, sid(2)), other, "{who}");
        }
        let mut locked = e.clone();
        locked.locked = true;
        assert!(
            !can_move_shape(v(P1), &locked, sid(1)),
            "the DM can lock movement"
        );
        assert!(
            !can_preview(v(P1), &e, sid(1)),
            "previews only when live dragging is on"
        );
        locked.live_drag = true;
        locked.locked = false;
        assert!(can_preview(v(P1), &locked, sid(1)));
        let template = |n: u8, owner: Option<UserId>| Shape {
            kind: ShapeKind::Template(beholder_core::combat::Template {
                area: beholder_core::combat::Area::Circle,
                feet: 20,
                angle: 0,
            }),
            ..shape(n, false, owner)
        };
        // (viewer, may place their own template, may change P1's template, may remove the token)
        let mut with_template = e.clone();
        with_template.shapes.push(template(5, Some(uid(P1))));
        for (who, own, others, token) in [
            (DM, true, true, true),
            (P1, true, true, false),
            (P2, true, false, false),
        ] {
            assert_eq!(
                can_put_shape(v(who), &with_template, &template(6, Some(uid(who)))),
                own,
                "{who}"
            );
            assert_eq!(
                can_put_shape(v(who), &with_template, &template(5, Some(uid(who)))),
                others,
                "{who}"
            );
            assert_eq!(
                can_remove_shape(v(who), &with_template, sid(5)),
                others,
                "{who}"
            );
            assert_eq!(
                can_remove_shape(v(who), &with_template, sid(1)),
                token,
                "{who}"
            );
        }
        assert!(
            !can_put_shape(v(P1), &with_template, &shape(6, false, Some(uid(P1)))),
            "players place templates, not tokens or terrain"
        );
        assert!(
            !can_put_shape(
                v(P1),
                &with_template,
                &Shape {
                    hidden: true,
                    ..template(6, Some(uid(P1)))
                }
            ),
            "players can't hide their templates"
        );
        let mut crowded = with_template.clone();
        crowded
            .shapes
            .extend((10..10 + MAX_PLAYER_TEMPLATES as u8).map(|n| template(n, Some(uid(P2)))));
        assert!(
            !can_put_shape(v(P2), &crowded, &template(40, Some(uid(P2)))),
            "a cap per player"
        );
        let hero = Character::new(
            beholder_core::CharacterId(Uuid::from_bytes([7; 16])),
            uid(P1),
            "Hero".into(),
            beholder_core::compendium::RulesVersion::R2024,
        );
        // (viewer, may act for the PC combatant, may act for a monster)
        for (who, pc, monster) in [(DM, true, true), (P1, true, false), (P2, false, false)] {
            assert_eq!(
                acts_for(v(who), &e, cid(1), std::slice::from_ref(&hero)),
                pc,
                "{who}"
            );
            assert_eq!(
                acts_for(v(who), &e, cid(2), std::slice::from_ref(&hero)),
                monster,
                "{who}"
            );
        }
        assert!(
            can_preview_as(v(DM)) && !can_preview_as(v(P1)),
            "only the DM previews a player's view"
        );
        let mut shut = with_template.clone();
        shut.locked = true;
        assert!(
            !can_put_shape(v(P1), &shut, &template(6, Some(uid(P1)))),
            "locking stops templates too"
        );
        let mut map = HashMap::new();
        map.insert(e.id, e.clone());
        let scope = Scope {
            settings: settings(CombatVisibility::Exact),
            notes: &no_notes(),
            encounters: &map,
        };
        let moved = |n| CampaignEvent::ShapeMoved {
            encounter: e.id,
            shape: sid(n),
            x: 2,
            y: 2,
        };
        assert!(event_for(v(P2), &scope, &moved(1)).is_some());
        assert!(
            event_for(v(P2), &scope, &moved(2)).is_none(),
            "moves of hidden shapes stay with the DM"
        );
        assert!(event_for(v(DM), &scope, &moved(2)).is_some());
        let saved = CampaignEvent::EncounterSaved {
            encounter: Box::new(idle),
        };
        assert!(matches!(
            event_for(v(P1), &scope, &saved),
            Some(CampaignEvent::EncounterDeleted { .. })
        ));
    }

    #[test]
    fn roll_matrix() {
        use beholder_core::dice::{Expr, roll};
        let record = |roller: u8, visibility: Visibility| {
            let expr = Expr::parse("1d20").unwrap();
            RollRecord {
                id: beholder_core::CommandId(Uuid::from_bytes([roller; 16])),
                roller: uid(roller),
                at: UnixMillis(0),
                character: None,
                context: String::new(),
                outcome: roll(&expr, |_| 10),
                expr,
                visibility,
                physical: false,
                targets: Vec::new(),
            }
        };
        // (roller, visibility, [DM sees, P1 sees, P2 sees])
        let rows = [
            (P1, Visibility::Public, [true, true, true]),
            (P1, Visibility::ToDm, [true, true, false]),
            (DM, Visibility::ToDm, [true, false, false]),
        ];
        for (roller, visibility, want) in rows {
            let r = record(roller, visibility);
            for (who, see) in [DM, P1, P2].into_iter().zip(want) {
                assert_eq!(
                    can_see_roll(v(who), &r),
                    see,
                    "{roller} {visibility:?} {who}"
                );
                let event = CampaignEvent::Rolled {
                    roll: Box::new(r.clone()),
                };
                assert_eq!(
                    event_for(v(who), &scope(&no_notes(), false), &event).is_some(),
                    see
                );
            }
        }
    }

    #[test]
    fn character_matrix() {
        let mut ch = Character::new(
            beholder_core::CharacterId(Uuid::from_bytes([9; 16])),
            uid(P1),
            "Ilsa".into(),
            beholder_core::compendium::RulesVersion::R2024,
        );
        ch.notes = "owes the thieves' guild".into();
        ch.hp.current = 7;
        let mut campaign = Campaign::new(
            beholder_core::CampaignId(Uuid::from_bytes([5; 16])),
            CampaignTitle::new("Test").unwrap(),
            member(DM, Role::Dm),
        );
        campaign.characters.push(ch.clone());
        let saved = CampaignEvent::CharacterSaved {
            character: Box::new(ch.clone()),
        };
        // (viewer, sees and edits the full sheet)
        for (who, full) in [(DM, true), (P1, true), (P2, false)] {
            assert_eq!(can_edit_character(v(who), &ch), full);
            let filtered = campaign_for(v(who), &campaign);
            match event_for(v(who), &scope(&no_notes(), false), &saved) {
                Some(CampaignEvent::CharacterSaved { character }) => {
                    assert!(full && character.notes == ch.notes);
                    assert_eq!(filtered.characters, vec![ch.clone()]);
                    assert!(filtered.party.is_empty());
                }
                Some(CampaignEvent::PartyMemberSaved { summary }) => {
                    assert!(!full);
                    assert_eq!((summary.name.as_str(), summary.hp.current), ("Ilsa", 7));
                    assert!(filtered.characters.is_empty(), "no full sheet leaks");
                    assert_eq!(filtered.party, vec![ch.summary()]);
                }
                other => panic!("unexpected {other:?}"),
            }
        }
    }

    #[test]
    fn library_edit_matrix() {
        let (author, other) = (uid(P1), uid(P2));
        assert!(
            can_edit_library(author, author, false),
            "authors edit their entries"
        );
        assert!(
            !can_edit_library(other, author, false),
            "other players can't"
        );
        assert!(can_edit_library(other, author, true), "any DM can tidy up");
        assert!(
            can_undo_import(author, author, false),
            "importers undo their imports"
        );
        assert!(
            !can_undo_import(other, author, false),
            "other players can't"
        );
        assert!(can_undo_import(other, author, true), "any DM can");
    }

    #[test]
    fn note_edit_and_manage_matrix() {
        // (note, [DM, P1, P2] may edit); only the author manages.
        let rows = [
            (note(P1, Audience::Private), [false, true, false]),
            (note(P1, Audience::Party), [true, true, true]),
            (
                note(P1, Audience::Users(vec![uid(P2)])),
                [false, true, true],
            ),
            (note(DM, Audience::Private), [true, false, false]),
        ];
        for (n, edit) in rows {
            for (who, want) in [DM, P1, P2].into_iter().zip(edit) {
                assert_eq!(
                    can_edit_note(v(who), &n),
                    want,
                    "edit {who} {:?}",
                    n.audience
                );
                assert_eq!(can_manage_note(v(who), &n), n.author == uid(who));
            }
        }
        let private = note(P1, Audience::Private);
        let dm = v(DM);
        let notes = no_notes();
        assert!(can_read_note(dm, &scope(&notes, true).settings, &private));
        assert!(!can_edit_note(dm, &private), "DM read access is read-only");
    }

    #[test]
    fn reveal_upserts_for_gainers_and_removes_for_losers() {
        let revealed = note(DM, Audience::Users(vec![uid(P1)]));
        let index = HashMap::from([(revealed.id, revealed.clone())]);
        let sc = scope(&index, false);
        let change = CampaignEvent::NoteAudienceChanged {
            id: revealed.id,
            audience: Audience::Users(vec![uid(P1)]),
            previous: Audience::Users(vec![uid(P2)]),
        };
        assert_eq!(event_for(v(DM), &sc, &change), Some(change.clone()));
        assert_eq!(
            event_for(v(P1), &sc, &change),
            Some(CampaignEvent::NoteCreated {
                note: revealed.clone()
            })
        );
        assert_eq!(
            event_for(v(P2), &sc, &change),
            Some(CampaignEvent::NoteDeleted { id: revealed.id })
        );
        let stranger = Viewer {
            user: uid(9),
            role: Role::Player,
        };
        assert_eq!(event_for(stranger, &sc, &change), None);
        assert_eq!(
            event_for(v(P1), &scope(&no_notes(), false), &change),
            None,
            "unknown note: nothing is sent"
        );
    }

    #[test]
    fn snapshot_filters_notes_per_viewer() {
        let mut c = Campaign::new(
            beholder_core::CampaignId(Uuid::from_bytes([3; 16])),
            CampaignTitle::new("T").unwrap(),
            member(DM, Role::Dm),
        );
        for (n, _, _) in read_matrix().into_iter().take(7) {
            c.apply(&CampaignEvent::NoteCreated { note: n });
        }
        for dm_reads in [false, true] {
            c.settings.dm_can_read_private_notes = dm_reads;
            for who in [DM, P1, P2] {
                let got = campaign_for(v(who), &c);
                let want: Vec<_> = c
                    .notes
                    .iter()
                    .filter(|n| can_read_note(v(who), &c.settings, n))
                    .cloned()
                    .collect();
                assert_eq!(got.notes, want);
                let leaked = got.notes.iter().any(|n| {
                    n.audience == Audience::Private
                        && n.author != uid(who)
                        && !(who == DM && dm_reads)
                });
                assert!(!leaked, "viewer {who} got someone else's private note");
            }
        }
    }

    fn member(n: u8, role: Role) -> Member {
        Member {
            user_id: UserId(Uuid::from_bytes([n; 16])),
            display_name: DisplayName::new("M").unwrap(),
            role,
        }
    }

    fn code() -> InviteCode {
        InviteCode::from_random([1, 2, 3, 4, 5])
    }

    fn op_matrix() -> Vec<(CampaignOp, bool, bool)> {
        vec![
            (
                CampaignOp::Rename {
                    title: CampaignTitle::new("T").unwrap(),
                },
                true,
                false,
            ),
            (CampaignOp::SetCover { cover: None }, true, false),
            (CampaignOp::RegenerateInvite, true, false),
            (
                CampaignOp::UpdateSettings(CampaignSettings::default()),
                true,
                false,
            ),
            (
                CampaignOp::SetSources {
                    sources: vec!["tob3".into()],
                },
                true,
                false,
            ),
            (
                CampaignOp::HideEntry {
                    id: beholder_core::LibraryId(Uuid::from_bytes([7; 16])),
                    hidden: true,
                },
                true,
                false,
            ),
        ]
    }

    fn event_matrix() -> Vec<(CampaignEvent, bool, bool)> {
        vec![
            (
                CampaignEvent::Created {
                    title: CampaignTitle::new("T").unwrap(),
                    dm: member(1, Role::Dm),
                },
                true,
                true,
            ),
            (
                CampaignEvent::Renamed {
                    title: CampaignTitle::new("T").unwrap(),
                },
                true,
                true,
            ),
            (
                CampaignEvent::CoverChanged {
                    cover: Some(ImageHash([1; 32])),
                },
                true,
                true,
            ),
            (
                CampaignEvent::MemberJoined {
                    member: member(2, Role::Player),
                },
                true,
                true,
            ),
            (
                CampaignEvent::InviteRegenerated { code: code() },
                true,
                false,
            ),
            (
                CampaignEvent::SettingsChanged {
                    settings: CampaignSettings {
                        dm_can_read_private_notes: true,
                        combat_visibility: CombatVisibility::Vague,
                    },
                },
                true,
                true,
            ),
            (
                CampaignEvent::SourcesChanged {
                    sources: vec!["tob3".into()],
                },
                true,
                true,
            ),
            (
                CampaignEvent::EntryHidden {
                    id: beholder_core::LibraryId(Uuid::from_bytes([7; 16])),
                    hidden: true,
                },
                true,
                true,
            ),
        ]
    }

    #[test]
    fn op_permissions() {
        for (op, dm, player) in op_matrix() {
            assert_eq!(can_apply(Role::Dm, &op), dm, "DM {op:?}");
            assert_eq!(can_apply(Role::Player, &op), player, "player {op:?}");
        }
    }

    #[test]
    fn event_visibility() {
        for (event, dm, player) in event_matrix() {
            for (role, expected) in [(Role::Dm, dm), (Role::Player, player)] {
                let notes = no_notes();
                let got = event_for(viewer(role), &scope(&notes, false), &event);
                assert_eq!(got.is_some(), expected, "{role:?} {event:?}");
                if let Some(got) = got {
                    assert_eq!(got, event, "visible events are passed unchanged");
                }
            }
        }
    }

    #[test]
    fn snapshot_hides_invite_code_from_players() {
        let mut c = Campaign::new(
            beholder_core::CampaignId(Uuid::from_bytes([3; 16])),
            CampaignTitle::new("T").unwrap(),
            member(1, Role::Dm),
        );
        c.invite_code = Some(code());
        assert_eq!(campaign_for(viewer(Role::Dm), &c).invite_code, Some(code()));
        assert_eq!(campaign_for(viewer(Role::Player), &c).invite_code, None);
        let mut rest = campaign_for(viewer(Role::Player), &c);
        rest.invite_code = c.invite_code;
        assert_eq!(rest, c, "nothing else is hidden from players");
    }

    #[test]
    fn outgoing_events_filters_and_drops_empty() {
        let id = beholder_core::CampaignId(Uuid::from_bytes([3; 16]));
        let envs: Vec<_> = event_matrix()
            .into_iter()
            .enumerate()
            .map(|(i, (event, _, _))| EventEnvelope {
                seq: Seq(i as u64 + 1),
                ts: UnixMillis(0),
                actor: member(1, Role::Dm).user_id,
                event,
            })
            .collect();
        for role in ROLES {
            let notes = no_notes();
            let Some(out) = Outgoing::events(viewer(role), &scope(&notes, false), id, &envs) else {
                panic!("{role:?} sees some events");
            };
            let ServerMsg::Events { events, .. } = out.msg() else {
                panic!("not an Events message");
            };
            let has_code = events
                .iter()
                .any(|e| matches!(e.event, CampaignEvent::InviteRegenerated { .. }));
            assert_eq!(has_code, role == Role::Dm);
        }
        let only_code = [EventEnvelope {
            seq: Seq(1),
            ts: UnixMillis(0),
            actor: member(1, Role::Dm).user_id,
            event: CampaignEvent::InviteRegenerated { code: code() },
        }];
        let notes = no_notes();
        assert_eq!(
            Outgoing::events(viewer(Role::Player), &scope(&notes, false), id, &only_code),
            None
        );
    }

    #[test]
    fn control_refuses_campaign_data() {
        let c = Campaign::new(
            beholder_core::CampaignId(Uuid::from_bytes([3; 16])),
            CampaignTitle::new("T").unwrap(),
            member(1, Role::Dm),
        );
        assert!(
            Outgoing::control(ServerMsg::Snapshot {
                seq: Seq(1),
                campaign: c.clone(),
            })
            .is_none()
        );
        assert!(
            Outgoing::control(ServerMsg::Events {
                campaign_id: c.id,
                events: vec![],
            })
            .is_none()
        );
        assert!(Outgoing::control(ServerMsg::HelloErr(HelloError::InvalidSession)).is_some());
    }
}
