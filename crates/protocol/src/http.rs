use beholder_core::{DisplayName, ImageHash, Password, User, Username};
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

pub const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;

pub const MAX_IMAGE_PIXELS: u64 = 40_000_000;
