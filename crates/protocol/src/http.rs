use beholder_core::compendium::{GameSystem, RulesVersion};
use beholder_core::library::LibraryEvent;
use beholder_core::{
    DisplayName, ImageHash, ImportId, Password, UnixMillis, User, UserId, Username,
};
use serde::{Deserialize, Serialize};

use crate::SessionToken;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub username: Username,
    pub display_name: DisplayName,
    pub password: Password,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: Username,
    pub password: Password,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: SessionToken,
    pub user: User,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthError {
    UsernameTaken,
    InvalidCredentials,
    RateLimited,
    UpdateRequired { server_version: u16 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageUploaded {
    pub hash: ImageHash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageError {
    TooLarge,
    TooManyPixels,
    Unsupported,
    Unauthorized,
    RateLimited,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceInfo {
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub system: GameSystem,
    pub rules: RulesVersion,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryChange {
    pub seq: u64,
    pub event: LibraryEvent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryChanges {
    pub head: u64,
    pub changes: Option<Vec<LibraryChange>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportInfo {
    pub id: ImportId,
    pub actor: UserId,
    pub actor_name: String,
    pub ts: UnixMillis,
    pub entries: u32,
    pub undone: bool,
    pub kept: u32,
}

pub const MAX_BUNDLE_BYTES: usize = 64 * 1024 * 1024;

pub const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;

pub const MAX_IMAGE_PIXELS: u64 = 40_000_000;
