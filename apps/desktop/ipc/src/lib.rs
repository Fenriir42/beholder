//! Types crossing the Tauri boundary between the UI (wasm32) and the shell.
//! They travel as JSON through `invoke` and events; they never go on the wire
//! to the server, so they can change with the UI without a protocol bump.

use beholder_core::{Campaign, User, UserId, ValidationError};
use beholder_protocol::RejectReason;
use beholder_protocol::http::{AuthError, ImageError};
use serde::{Deserialize, Serialize};

/// Event names emitted by the shell.
pub mod events {
    /// Payload: [`super::ConnState`].
    pub const STATE: &str = "beholder://state";
    /// No payload: call the `view` command again.
    pub const VIEW_CHANGED: &str = "beholder://view-changed";
    /// Payload: [`super::Rejected`].
    pub const REJECTED: &str = "beholder://rejected";
    /// No payload: the session ended; show the login screen.
    pub const SIGNED_OUT: &str = "beholder://signed-out";
    /// Payload: the server's protocol version (`u16`).
    pub const UPDATE_REQUIRED: &str = "beholder://update-required";
}

/// What the UI needs at startup to pick its first screen.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Boot {
    /// The remembered server, if any.
    pub server_url: Option<String>,
    /// A session exists (token in the keyring).
    pub signed_in: bool,
    /// The remembered UI language (`en` / `fr`).
    pub locale: Option<String>,
    /// Base URL for cached images; differs by platform (custom URI schemes).
    pub image_base: String,
}

/// Connection pill state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnState {
    /// First attempt.
    Connecting,
    /// Connected and synced.
    Online,
    /// Lost the connection, retrying.
    Reconnecting {
        /// Changes waiting to be sent.
        queued: usize,
    },
    /// Several attempts failed; showing cached data.
    Offline {
        /// Changes waiting to be sent.
        queued: usize,
    },
}

/// One campaign as the UI shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignView {
    /// State including pending changes.
    pub campaign: Campaign,
    /// Some shown changes are still waiting for the server.
    pub pending: bool,
    /// Members currently online.
    pub online: Vec<UserId>,
}

/// Everything the library and campaign screens render.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewData {
    /// The signed-in user.
    pub user: Option<User>,
    /// Campaigns, sorted by title.
    pub campaigns: Vec<CampaignView>,
    /// Join requests in flight.
    pub pending_joins: usize,
}

/// A command the server refused (already rolled back in the view).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejected {
    /// Why.
    pub reason: RejectReason,
}

/// Why a Tauri command failed. The UI maps each case to a translated message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IpcError {
    /// A field is invalid.
    Invalid(ValidationError),
    /// The server URL isn't `http(s)://host[:port]`.
    BadServerUrl,
    /// Login or registration refused.
    Auth(AuthError),
    /// Image refused.
    Image(ImageError),
    /// The server couldn't be reached.
    Network,
    /// The server answered something unexpected.
    Server,
    /// Not signed in.
    SignedOut,
    /// Local storage failed.
    Storage,
}
