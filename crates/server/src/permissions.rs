use std::collections::HashMap;

use beholder_core::character::Character;
use beholder_core::{
    Audience, Campaign, CampaignEvent, CampaignId, CampaignOp, CampaignSettings, Note, NoteId,
    Role, Seq, UserId,
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

pub struct Scope<'a> {
    pub settings: CampaignSettings,
    pub notes: &'a HashMap<NoteId, Note>,
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

    pub fn control(msg: ServerMsg) -> Option<Self> {
        match msg {
            ServerMsg::Events { .. } | ServerMsg::Snapshot { .. } => None,
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
