use std::collections::{HashMap, HashSet};

use beholder_core::library::LibraryOp;
use beholder_core::{Campaign, CampaignId, LibraryId, Member, NoteId, Role, Seq, User, UserId};
use beholder_protocol::{Command, CommandKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignView {
    pub campaign: Campaign,
    pub seq: Seq,
    pub pending: bool,
    pub pending_notes: Vec<NoteId>,
    pub online: Vec<UserId>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub user: Option<User>,
    pub campaigns: Vec<CampaignView>,
    pub pending_joins: usize,
    pub pending_library: Vec<LibraryId>,
}

pub fn compute(
    user: Option<User>,
    confirmed: Vec<(Seq, Campaign)>,
    outbox: &[Command],
    online: &HashMap<CampaignId, HashSet<UserId>>,
) -> View {
    let mut campaigns: Vec<CampaignView> = confirmed
        .into_iter()
        .map(|(seq, campaign)| CampaignView {
            campaign,
            seq,
            pending: false,
            pending_notes: Vec::new(),
            online: Vec::new(),
        })
        .collect();
    let mut pending_joins = 0;
    let mut pending_library = Vec::new();
    for cmd in outbox {
        match &cmd.kind {
            CommandKind::CreateCampaign { campaign_id, title } => {
                if campaigns.iter().any(|c| c.campaign.id == *campaign_id) {
                    continue;
                }
                let Some(me) = &user else { continue };
                let dm = Member {
                    user_id: me.id,
                    display_name: me.display_name.clone(),
                    role: Role::Dm,
                };
                campaigns.push(CampaignView {
                    campaign: Campaign::new(*campaign_id, title.clone(), dm),
                    seq: Seq(0),
                    pending: true,
                    pending_notes: Vec::new(),
                    online: Vec::new(),
                });
            }
            CommandKind::JoinCampaign { .. } => pending_joins += 1,
            CommandKind::Library { op } => pending_library.push(match op {
                LibraryOp::Save { draft, .. } => draft.id,
                LibraryOp::Delete { id } => *id,
            }),
            CommandKind::Campaign { campaign_id, op } => {
                let Some(view) = campaigns.iter_mut().find(|c| c.campaign.id == *campaign_id)
                else {
                    continue;
                };
                if let Some(event) = op.predicted_event() {
                    view.campaign.apply(&event);
                }
                view.pending = true;
            }
            CommandKind::Note { campaign_id, op } => {
                let (Some(me), Some(view)) = (
                    &user,
                    campaigns.iter_mut().find(|c| c.campaign.id == *campaign_id),
                ) else {
                    continue;
                };
                let id = op.note_id();
                if let Some(event) = op.predicted_event(me.id, view.campaign.note(id)) {
                    view.campaign.apply(&event);
                }
                if !view.pending_notes.contains(&id) {
                    view.pending_notes.push(id);
                }
            }
        }
    }
    for view in &mut campaigns {
        if let Some(set) = online.get(&view.campaign.id) {
            view.online = view
                .campaign
                .members
                .iter()
                .map(|m| m.user_id)
                .filter(|u| set.contains(u))
                .collect();
        }
    }
    campaigns.sort_by(|a, b| a.campaign.title.as_str().cmp(b.campaign.title.as_str()));
    View {
        user,
        campaigns,
        pending_joins,
        pending_library,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use beholder_core::{CampaignOp, CampaignTitle, CommandId, DisplayName, InviteCode, Username};
    use uuid::Uuid;

    fn me() -> User {
        User {
            id: UserId(Uuid::from_bytes([1; 16])),
            username: Username::new("me1".into()).unwrap(),
            display_name: DisplayName::new("Me").unwrap(),
        }
    }

    fn cmd(n: u8, kind: CommandKind) -> Command {
        Command {
            id: CommandId(Uuid::from_bytes([n; 16])),
            kind,
        }
    }

    fn confirmed(title: &str) -> (Seq, Campaign) {
        let u = me();
        (
            Seq(3),
            Campaign::new(
                CampaignId(Uuid::from_bytes([9; 16])),
                CampaignTitle::new(title).unwrap(),
                Member {
                    user_id: u.id,
                    display_name: u.display_name,
                    role: Role::Dm,
                },
            ),
        )
    }

    #[test]
    fn pending_ops_apply_on_top_and_vanish_when_removed() {
        let base = confirmed("Old");
        let id = base.1.id;
        let rename = cmd(
            1,
            CommandKind::Campaign {
                campaign_id: id,
                op: CampaignOp::Rename {
                    title: CampaignTitle::new("New").unwrap(),
                },
            },
        );
        let v = compute(Some(me()), vec![base.clone()], &[rename], &HashMap::new());
        assert_eq!(v.campaigns[0].campaign.title.as_str(), "New");
        assert!(v.campaigns[0].pending);
        let v = compute(Some(me()), vec![base], &[], &HashMap::new());
        assert_eq!(v.campaigns[0].campaign.title.as_str(), "Old");
        assert!(!v.campaigns[0].pending);
    }

    #[test]
    fn pending_create_and_join_are_shown() {
        let create = cmd(
            1,
            CommandKind::CreateCampaign {
                campaign_id: CampaignId(Uuid::from_bytes([5; 16])),
                title: CampaignTitle::new("Fresh").unwrap(),
            },
        );
        let join = cmd(
            2,
            CommandKind::JoinCampaign {
                code: InviteCode::from_random([1; 5]),
            },
        );
        let v = compute(Some(me()), vec![], &[create, join], &HashMap::new());
        assert_eq!(v.campaigns.len(), 1);
        assert_eq!(v.campaigns[0].seq, Seq(0));
        assert_eq!(v.campaigns[0].campaign.role_of(me().id), Some(Role::Dm));
        assert_eq!(v.pending_joins, 1);
    }

    #[test]
    fn pending_note_edits_chain_on_predicted_versions() {
        use beholder_core::{Audience, NoteBody, NoteOp, NoteTitle};
        let base = confirmed("A");
        let campaign_id = base.1.id;
        let id = NoteId(Uuid::from_bytes([4; 16]));
        let title = NoteTitle::new("Map").unwrap();
        let note = |n, op| cmd(n, CommandKind::Note { campaign_id, op });
        let edit = |base_version, text: &str| NoteOp::Edit {
            id,
            base_version,
            title: title.clone(),
            body: NoteBody::new(text.into()).unwrap(),
        };
        let outbox = [
            note(
                1,
                NoteOp::Create {
                    id,
                    title: title.clone(),
                    body: NoteBody::default(),
                    audience: Audience::Party,
                },
            ),
            note(2, edit(1, "first")),
            note(3, edit(2, "second")),
            note(4, edit(2, "stale")),
        ];
        let v = compute(Some(me()), vec![base], &outbox, &HashMap::new());
        let c = &v.campaigns[0];
        let n = c.campaign.note(id).unwrap();
        assert_eq!(n.body.as_str(), "second", "a stale edit isn't predicted");
        assert_eq!(n.version, 3);
        assert_eq!(n.author, me().id);
        assert_eq!(c.pending_notes, vec![id]);
        assert!(
            !c.pending,
            "note edits don't mark campaign settings pending"
        );
    }

    #[test]
    fn presence_is_limited_to_members() {
        let base = confirmed("A");
        let id = base.1.id;
        let stranger = UserId(Uuid::from_bytes([7; 16]));
        let online = HashMap::from([(id, HashSet::from([me().id, stranger]))]);
        let v = compute(Some(me()), vec![base], &[], &online);
        assert_eq!(v.campaigns[0].online, vec![me().id]);
    }
}
