use beholder_core::{Campaign, NoteId, User, UserId, ValidationError};
use beholder_protocol::RejectReason;
use beholder_protocol::http::{AuthError, ImageError};
use serde::{Deserialize, Serialize};

pub mod events {
    pub const STATE: &str = "beholder://state";
    pub const VIEW_CHANGED: &str = "beholder://view-changed";
    pub const REJECTED: &str = "beholder://rejected";
    pub const SIGNED_OUT: &str = "beholder://signed-out";
    pub const UPDATE_REQUIRED: &str = "beholder://update-required";
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Boot {
    pub server_url: Option<String>,
    pub signed_in: bool,
    pub locale: Option<String>,
    pub image_base: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnState {
    Connecting,
    Online,
    Reconnecting { queued: usize },
    Offline { queued: usize },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignView {
    pub campaign: Campaign,
    pub pending: bool,
    pub pending_notes: Vec<NoteId>,
    pub online: Vec<UserId>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewData {
    pub user: Option<User>,
    pub campaigns: Vec<CampaignView>,
    pub pending_joins: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejected {
    pub reason: RejectReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IpcError {
    Invalid(ValidationError),
    BadServerUrl,
    Auth(AuthError),
    Image(ImageError),
    Network,
    Server,
    SignedOut,
    Storage,
}
