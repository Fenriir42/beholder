//! Every permission rule of the server, as pure functions, plus [`Outgoing`]:
//! the only way to build a message carrying campaign data, so nothing reaches
//! a client without passing through the filters here.

use beholder_core::{Campaign, CampaignEvent, CampaignId, CampaignOp, Role, Seq, UserId};
use beholder_protocol::{EventEnvelope, ServerMsg};

/// Whether a member with `role` may apply `op` to their campaign.
pub fn can_apply(role: Role, op: &CampaignOp) -> bool {
    match op {
        CampaignOp::Rename { .. }
        | CampaignOp::SetCover { .. }
        | CampaignOp::RegenerateInvite
        | CampaignOp::UpdateSettings(_) => role == Role::Dm,
    }
}

/// The version of `event` a member with role `viewer` may receive, if any.
pub fn event_for(viewer: Role, event: &CampaignEvent) -> Option<CampaignEvent> {
    match event {
        // Players could hand the code to anyone; only the DM controls who joins.
        CampaignEvent::InviteRegenerated { .. } => (viewer == Role::Dm).then(|| event.clone()),
        CampaignEvent::Created { .. }
        | CampaignEvent::Renamed { .. }
        | CampaignEvent::CoverChanged { .. }
        | CampaignEvent::MemberJoined { .. }
        // Settings are deliberately visible to everyone (SPEC §4).
        | CampaignEvent::SettingsChanged { .. } => Some(event.clone()),
    }
}

/// The view of `campaign` a member with role `viewer` may receive.
pub fn campaign_for(viewer: Role, campaign: &Campaign) -> Campaign {
    let mut c = campaign.clone();
    if viewer != Role::Dm {
        c.invite_code = None;
    }
    c
}

/// A server message that has been filtered for its recipient.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outgoing(ServerMsg);

impl Outgoing {
    /// Events for one recipient; `None` when they may see none of them.
    pub fn events(viewer: Role, campaign_id: CampaignId, events: &[EventEnvelope]) -> Option<Self> {
        let events: Vec<_> = events
            .iter()
            .filter_map(|e| {
                event_for(viewer, &e.event).map(|event| EventEnvelope {
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

    /// A campaign snapshot for one recipient.
    pub fn snapshot(viewer: Role, seq: Seq, campaign: &Campaign) -> Self {
        Self(ServerMsg::Snapshot {
            seq,
            campaign: campaign_for(viewer, campaign),
        })
    }

    /// A message carrying no campaign data. `None` for `Events` and `Snapshot`,
    /// which must go through [`Outgoing::events`] / [`Outgoing::snapshot`].
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
            | ServerMsg::Pong { .. } => Some(Self(msg)),
        }
    }

    /// Who of a campaign's members is online. Presence is visible to all members.
    pub fn presence(campaign_id: CampaignId, online: Vec<UserId>) -> Self {
        Self(ServerMsg::Presence {
            campaign_id,
            online,
        })
    }

    /// One member came online or went offline.
    pub fn presence_changed(campaign_id: CampaignId, user_id: UserId, online: bool) -> Self {
        Self(ServerMsg::PresenceChanged {
            campaign_id,
            user_id,
            online,
        })
    }

    /// The message to encode and send.
    pub fn msg(&self) -> &ServerMsg {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    //! The permission matrix: every role against every op, event and view.

    use super::*;
    use beholder_core::{
        CampaignSettings, CampaignTitle, CombatVisibility, DisplayName, ImageHash, InviteCode,
        Member, UnixMillis, UserId,
    };
    use beholder_protocol::HelloError;
    use uuid::Uuid;

    const ROLES: [Role; 2] = [Role::Dm, Role::Player];

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

    /// (op, DM allowed, player allowed)
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
        ]
    }

    /// (event, DM sees it, player sees it)
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
                let got = event_for(role, &event);
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
        assert_eq!(campaign_for(Role::Dm, &c).invite_code, Some(code()));
        assert_eq!(campaign_for(Role::Player, &c).invite_code, None);
        let mut rest = campaign_for(Role::Player, &c);
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
            let Some(out) = Outgoing::events(role, id, &envs) else {
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
        assert_eq!(Outgoing::events(Role::Player, id, &only_code), None);
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
