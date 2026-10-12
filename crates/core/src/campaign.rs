//! Campaigns, membership, settings, invite codes, and the campaign event log's pure state machine.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::{CampaignId, ImageHash, UserId};
use crate::user::{DisplayName, ValidationError, clean_text};

/// A member's role in one campaign. Each campaign has exactly one DM.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    /// Dungeon master: owns the campaign and its settings.
    Dm,
    /// Player.
    Player,
}

/// What players see of monsters and NPCs in the combat tracker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CombatVisibility {
    /// Everything, including exact HP.
    #[default]
    Exact,
    /// Turn order and conditions; HP shown as Healthy / Bloodied / Near death.
    Vague,
    /// Players don't see the tracker.
    Hidden,
}

/// DM-controlled campaign rules, readable by every member.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CampaignSettings {
    /// When on, the DM also receives players' private notes.
    pub dm_can_read_private_notes: bool,
    /// Player view of the combat tracker.
    pub combat_visibility: CombatVisibility,
}

/// Campaign title: 1–80 characters after trimming, no control characters.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CampaignTitle(String);

impl CampaignTitle {
    /// Trims and validates a title.
    pub fn new(s: &str) -> Result<Self, ValidationError> {
        clean_text(s, 80).map(Self)
    }

    /// The title.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CampaignTitle {
    type Error = ValidationError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(&s)
    }
}

impl From<CampaignTitle> for String {
    fn from(t: CampaignTitle) -> Self {
        t.0
    }
}

/// Crockford base32: no I, L, O or U, so codes survive being read aloud.
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// An 8-character invite code (40 random bits), shown as `ABCD-EFGH`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "[u8; 8]", into = "[u8; 8]")]
pub struct InviteCode([u8; 8]);

impl InviteCode {
    /// Builds a code from 5 random bytes; the caller supplies the randomness.
    pub fn from_random(bytes: [u8; 5]) -> Self {
        let mut n = bytes.iter().fold(0u64, |acc, &b| (acc << 8) | u64::from(b));
        let mut out = [0u8; 8];
        for c in out.iter_mut().rev() {
            *c = CROCKFORD[(n & 31) as usize];
            n >>= 5;
        }
        Self(out)
    }

    /// Parses what a user typed: case-insensitive, ignores spaces and dashes,
    /// and reads I/L as 1 and O as 0.
    pub fn parse(input: &str) -> Result<Self, ValidationError> {
        let mut out = [0u8; 8];
        let mut len = 0;
        for c in input.bytes() {
            let c = match c.to_ascii_uppercase() {
                b' ' | b'-' => continue,
                b'I' | b'L' => b'1',
                b'O' => b'0',
                c => c,
            };
            if !CROCKFORD.contains(&c) {
                return Err(ValidationError::Character);
            }
            let slot = out.get_mut(len).ok_or(ValidationError::Length)?;
            *slot = c;
            len += 1;
        }
        if len != 8 {
            return Err(ValidationError::Length);
        }
        Ok(Self(out))
    }

    /// The canonical 8 characters without separator, as stored.
    pub fn as_str(&self) -> &str {
        // Only CROCKFORD bytes ever get in, so this is always ASCII.
        core::str::from_utf8(&self.0).unwrap_or_default()
    }
}

impl TryFrom<[u8; 8]> for InviteCode {
    type Error = ValidationError;
    fn try_from(b: [u8; 8]) -> Result<Self, Self::Error> {
        if b.iter().all(|c| CROCKFORD.contains(c)) {
            Ok(Self(b))
        } else {
            Err(ValidationError::Character)
        }
    }
}

impl From<InviteCode> for [u8; 8] {
    fn from(c: InviteCode) -> Self {
        c.0
    }
}

impl fmt::Display for InviteCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.as_str();
        write!(f, "{}-{}", &s[..4], &s[4..])
    }
}

impl fmt::Debug for InviteCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "InviteCode({self})")
    }
}

/// A campaign member as other members see them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    /// Account id.
    pub user_id: UserId,
    /// Name shown to others.
    pub display_name: DisplayName,
    /// Role in this campaign.
    pub role: Role,
}

/// A change a member asks for inside an existing campaign.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CampaignOp {
    /// Change the title (DM).
    Rename {
        /// New title.
        title: CampaignTitle,
    },
    /// Set or clear the cover image (DM).
    SetCover {
        /// An image already uploaded over HTTPS, or `None` to clear.
        cover: Option<ImageHash>,
    },
    /// Replace the invite code, so the old one stops working (DM).
    RegenerateInvite,
    /// Replace the campaign settings (DM).
    UpdateSettings(CampaignSettings),
}

impl CampaignOp {
    /// The event the server will most likely emit, for optimistic display.
    /// `None` when the result depends on the server (e.g. a random code).
    pub fn predicted_event(&self) -> Option<CampaignEvent> {
        match self {
            Self::Rename { title } => Some(CampaignEvent::Renamed {
                title: title.clone(),
            }),
            Self::SetCover { cover } => Some(CampaignEvent::CoverChanged { cover: *cover }),
            Self::RegenerateInvite => None,
            Self::UpdateSettings(s) => Some(CampaignEvent::SettingsChanged { settings: *s }),
        }
    }
}

/// A state change recorded in a campaign's event log.
///
/// Stored as postcard in the server database: never change an existing
/// variant, only append new ones (see ADR-0003).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CampaignEvent {
    /// The campaign was created by its DM.
    Created {
        /// Initial title.
        title: CampaignTitle,
        /// The creating DM.
        dm: Member,
    },
    /// The title changed.
    Renamed {
        /// New title.
        title: CampaignTitle,
    },
    /// The cover changed or was cleared.
    CoverChanged {
        /// New cover.
        cover: Option<ImageHash>,
    },
    /// A user joined, or an existing member's details changed.
    MemberJoined {
        /// The member.
        member: Member,
    },
    /// A new invite code replaced the old one. DM-only.
    InviteRegenerated {
        /// The new code.
        code: InviteCode,
    },
    /// The DM changed the settings.
    SettingsChanged {
        /// New settings.
        settings: CampaignSettings,
    },
}

/// The current state of a campaign, as built by applying its events.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Campaign {
    /// Campaign id.
    pub id: CampaignId,
    /// Title.
    pub title: CampaignTitle,
    /// Cover image, if any.
    pub cover: Option<ImageHash>,
    /// DM-controlled settings.
    pub settings: CampaignSettings,
    /// Current invite code. Only ever `Some` in a DM's view.
    pub invite_code: Option<InviteCode>,
    /// Members in join order; the DM is first.
    pub members: Vec<Member>,
}

impl Campaign {
    /// A freshly created campaign with its DM as the only member.
    pub fn new(id: CampaignId, title: CampaignTitle, dm: Member) -> Self {
        Self {
            id,
            title,
            cover: None,
            settings: CampaignSettings::default(),
            invite_code: None,
            members: vec![dm],
        }
    }

    /// Applies one event. Pure and deterministic, shared by server and client.
    pub fn apply(&mut self, event: &CampaignEvent) {
        match event {
            CampaignEvent::Created { title, dm } => {
                *self = Self::new(self.id, title.clone(), dm.clone());
            }
            CampaignEvent::Renamed { title } => self.title = title.clone(),
            CampaignEvent::CoverChanged { cover } => self.cover = *cover,
            CampaignEvent::MemberJoined { member } => {
                match self
                    .members
                    .iter_mut()
                    .find(|m| m.user_id == member.user_id)
                {
                    Some(m) => *m = member.clone(),
                    None => self.members.push(member.clone()),
                }
            }
            CampaignEvent::InviteRegenerated { code } => self.invite_code = Some(*code),
            CampaignEvent::SettingsChanged { settings } => self.settings = *settings,
        }
    }

    /// The role of `user` in this campaign, if they are a member.
    pub fn role_of(&self, user: UserId) -> Option<Role> {
        self.members
            .iter()
            .find(|m| m.user_id == user)
            .map(|m| m.role)
    }

    /// The DM. `None` only for a corrupt state with no DM.
    pub fn dm(&self) -> Option<&Member> {
        self.members.iter().find(|m| m.role == Role::Dm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn member(n: u8, role: Role) -> Member {
        Member {
            user_id: UserId(Uuid::from_bytes([n; 16])),
            display_name: DisplayName::new(&format!("user{n}")).unwrap(),
            role,
        }
    }

    #[test]
    fn invite_code_generation_is_canonical() {
        let c = InviteCode::from_random([0, 0, 0, 0, 0]);
        assert_eq!(c.as_str(), "00000000");
        let c = InviteCode::from_random([0xff; 5]);
        assert_eq!(c.as_str(), "ZZZZZZZZ");
        let c = InviteCode::from_random([0x12, 0x34, 0x56, 0x78, 0x9a]);
        assert_eq!(InviteCode::parse(c.as_str()), Ok(c));
        assert_eq!(InviteCode::parse(&c.to_string()), Ok(c));
    }

    #[test]
    fn invite_code_parse_is_forgiving_but_strict_on_length() {
        let c = InviteCode::parse("AB1D-EF0H").unwrap();
        assert_eq!(InviteCode::parse("abld efoh"), Ok(c));
        assert_eq!(InviteCode::parse("aBiD-eFoH"), Ok(c));
        assert_eq!(c.to_string(), "AB1D-EF0H");
        assert_eq!(InviteCode::parse("ABCDEFG"), Err(ValidationError::Length));
        assert_eq!(InviteCode::parse("ABCDEFGHJ"), Err(ValidationError::Length));
        assert_eq!(
            InviteCode::parse("ABCDEFGU"),
            Err(ValidationError::Character)
        );
        assert_eq!(
            InviteCode::try_from(*b"abcdefgh"),
            Err(ValidationError::Character)
        );
    }

    #[test]
    fn title_validation() {
        assert_eq!(CampaignTitle::new("  Curse ").unwrap().as_str(), "Curse");
        assert_eq!(CampaignTitle::new(""), Err(ValidationError::Length));
        assert_eq!(
            CampaignTitle::new(&"x".repeat(81)),
            Err(ValidationError::Length)
        );
    }

    #[test]
    fn events_fold_into_state() {
        let id = CampaignId(Uuid::from_bytes([9; 16]));
        let dm = member(1, Role::Dm);
        let p = member(2, Role::Player);
        let mut c = Campaign::new(id, CampaignTitle::new("A").unwrap(), dm.clone());
        let code = InviteCode::from_random([1, 2, 3, 4, 5]);
        let settings = CampaignSettings {
            dm_can_read_private_notes: true,
            combat_visibility: CombatVisibility::Vague,
        };
        for e in [
            CampaignEvent::Renamed {
                title: CampaignTitle::new("B").unwrap(),
            },
            CampaignEvent::CoverChanged {
                cover: Some(ImageHash([7; 32])),
            },
            CampaignEvent::MemberJoined { member: p.clone() },
            CampaignEvent::MemberJoined { member: p.clone() },
            CampaignEvent::InviteRegenerated { code },
            CampaignEvent::SettingsChanged { settings },
        ] {
            c.apply(&e);
        }
        assert_eq!(c.title.as_str(), "B");
        assert_eq!(c.cover, Some(ImageHash([7; 32])));
        assert_eq!(c.members, vec![dm.clone(), p.clone()]);
        assert_eq!(c.invite_code, Some(code));
        assert_eq!(c.settings, settings);
        assert_eq!(c.role_of(p.user_id), Some(Role::Player));
        assert_eq!(c.dm(), Some(&dm));
        assert_eq!(c.role_of(UserId(Uuid::nil())), None);
    }

    #[test]
    fn predicted_events_match_ops() {
        let title = CampaignTitle::new("X").unwrap();
        assert_eq!(
            CampaignOp::Rename {
                title: title.clone()
            }
            .predicted_event(),
            Some(CampaignEvent::Renamed { title })
        );
        assert_eq!(CampaignOp::RegenerateInvite.predicted_event(), None);
    }
}
