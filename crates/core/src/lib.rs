//! Domain types and pure rules for Beholder. No IO, no async; builds for wasm32.

pub mod campaign;
pub mod ids;
pub mod user;

pub use campaign::{
    Campaign, CampaignEvent, CampaignOp, CampaignSettings, CampaignTitle, CombatVisibility,
    InviteCode, Member, Role,
};
pub use ids::{CampaignId, CommandId, ImageHash, Seq, UnixMillis, UserId};
pub use user::{DisplayName, Password, User, Username, ValidationError};
