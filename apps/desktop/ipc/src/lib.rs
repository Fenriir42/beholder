use beholder_core::compendium::{Alternative, Entry, Kind, LicenseText, SourceMeta};
use beholder_core::mapping::Profile;
use beholder_core::paste::Pasted;
use beholder_core::{Campaign, CharacterId, LibraryId, NoteId, User, UserId, ValidationError};
use beholder_protocol::RejectReason;
use beholder_protocol::http::{AuthError, ImageError};
use serde::{Deserialize, Serialize};

pub mod events {
    pub const STATE: &str = "beholder://state";
    pub const VIEW_CHANGED: &str = "beholder://view-changed";
    pub const REJECTED: &str = "beholder://rejected";
    pub const SIGNED_OUT: &str = "beholder://signed-out";
    pub const UPDATE_REQUIRED: &str = "beholder://update-required";
    pub const COMPENDIUM: &str = "beholder://compendium";
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
    pub pending_characters: Vec<CharacterId>,
    pub online: Vec<UserId>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewData {
    pub user: Option<User>,
    pub campaigns: Vec<CampaignView>,
    pub pending_joins: usize,
    pub pending_library: Vec<LibraryId>,
    pub pending_import: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejected {
    pub reason: RejectReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompendiumStatus {
    Syncing,
    Ready,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryView {
    pub entry: Entry,
    pub source: SourceMeta,
    pub also: Vec<Alternative>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credits {
    pub sources: Vec<SourceMeta>,
    pub licenses: Vec<LicenseText>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasteItem {
    pub entry: Pasted,
    pub duplicate: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkOpened {
    pub file_name: String,
    pub bytes: u64,
    pub keys: Vec<String>,
    pub sample: Vec<String>,
    pub profile: Profile,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkScanView {
    pub total: usize,
    pub kinds: Vec<(Kind, usize)>,
    pub books: Vec<(String, usize)>,
    pub duplicates: usize,
    pub partial: usize,
    pub invalid: Vec<String>,
    pub invalid_count: usize,
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
    BadFile,
}
