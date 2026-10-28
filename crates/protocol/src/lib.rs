// Postcard: only appending enum variants is compatible; anything else bumps PROTOCOL_VERSION (ADR-0003).

#[cfg(feature = "codec")]
pub mod codec;
pub mod http;

use core::fmt;

use beholder_core::character::CharacterOp;
use beholder_core::library::LibraryOp;
use beholder_core::{
    Campaign, CampaignEvent, CampaignId, CampaignOp, CampaignTitle, CommandId, InviteCode, NoteOp,
    Seq, UnixMillis, User, UserId, ValidationError,
};
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 6;

pub const PROTOCOL_HEADER: &str = "x-beholder-protocol";

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionToken(pub [u8; 32]);

impl SessionToken {
    pub fn to_hex(&self) -> String {
        beholder_core::ImageHash(self.0).to_hex()
    }

    pub fn from_hex(s: &str) -> Option<Self> {
        beholder_core::ImageHash::from_hex(s).map(|h| Self(h.0))
    }
}

impl fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionToken(***)")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientMsg {
    Hello(Hello),
    Command(Command),
    Ping { t: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol_version: u16,
    pub session_token: SessionToken,
    pub known: Vec<(CampaignId, Seq)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    pub id: CommandId,
    pub kind: CommandKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandKind {
    CreateCampaign {
        campaign_id: CampaignId,
        title: CampaignTitle,
    },
    JoinCampaign {
        code: InviteCode,
    },
    Campaign {
        campaign_id: CampaignId,
        op: CampaignOp,
    },
    Note {
        campaign_id: CampaignId,
        op: NoteOp,
    },
    Library {
        op: LibraryOp,
    },
    Character {
        campaign_id: CampaignId,
        op: CharacterOp,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerMsg {
    HelloOk(HelloOk),
    HelloErr(HelloError),
    Ack {
        id: CommandId,
        seq: Option<Seq>,
    },
    Reject {
        id: CommandId,
        reason: RejectReason,
    },
    Events {
        campaign_id: CampaignId,
        events: Vec<EventEnvelope>,
    },
    Snapshot {
        seq: Seq,
        campaign: Campaign,
    },
    CampaignRemoved {
        campaign_id: CampaignId,
    },
    Presence {
        campaign_id: CampaignId,
        online: Vec<UserId>,
    },
    PresenceChanged {
        campaign_id: CampaignId,
        user_id: UserId,
        online: bool,
    },
    Pong {
        t: u64,
    },
    LibraryChanged {
        seq: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloOk {
    pub user: User,
    pub campaigns: Vec<CampaignId>,
    pub rotated_token: Option<SessionToken>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HelloError {
    UpdateRequired { server_version: u16 },
    InvalidSession,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectReason {
    Forbidden,
    NotFound,
    InvalidInviteCode,
    Invalid(ValidationError),
    UnknownImage,
    IdConflict,
    Expired,
    RateLimited,
    Stale,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub seq: Seq,
    pub ts: UnixMillis,
    pub actor: UserId,
    pub event: CampaignEvent,
}
