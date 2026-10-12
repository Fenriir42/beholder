//! The optimistic view: confirmed state with pending commands applied on top.
//!
//! Rolling back a rejected command is just recomputing without it, and server
//! events rebase pending commands for free (ADR-0004).

use std::collections::{HashMap, HashSet};

use beholder_core::{Campaign, CampaignId, Member, Role, Seq, User, UserId};
use beholder_protocol::{Command, CommandKind};

/// What the UI shows for one campaign.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignView {
    /// State including pending changes.
    pub campaign: Campaign,
    /// Last confirmed seq (0 for a campaign that only exists locally so far).
    pub seq: Seq,
    /// Whether some of what's shown is still waiting for the server.
    pub pending: bool,
    /// Members currently online.
    pub online: Vec<UserId>,
}

/// Everything the UI needs, computed from the cache.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    /// The logged-in user, if known.
    pub user: Option<User>,
    /// Campaigns, sorted by title.
    pub campaigns: Vec<CampaignView>,
    /// Join requests still waiting for the server.
    pub pending_joins: usize,
}

/// Builds the view from confirmed state, the outbox and presence.
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
            online: Vec::new(),
        })
        .collect();
    let mut pending_joins = 0;
    for cmd in outbox {
        match &cmd.kind {
            CommandKind::CreateCampaign { campaign_id, title } => {
                // Already confirmed if the server's snapshot arrived before the Ack.
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
                    online: Vec::new(),
                });
            }
            CommandKind::JoinCampaign { .. } => pending_joins += 1,
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
        // Rejected: the command leaves the outbox and the view rolls back.
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
    fn presence_is_limited_to_members() {
        let base = confirmed("A");
        let id = base.1.id;
        let stranger = UserId(Uuid::from_bytes([7; 16]));
        let online = HashMap::from([(id, HashSet::from([me().id, stranger]))]);
        let v = compute(Some(me()), vec![base], &[], &online);
        assert_eq!(v.campaigns[0].online, vec![me().id]);
    }
}
