//! Postcard bodies for the HTTPS endpoints (auth and image upload).

use beholder_core::{DisplayName, ImageHash, Password, User, Username};
use serde::{Deserialize, Serialize};

use crate::SessionToken;

/// `POST /api/register`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterRequest {
    /// Login name.
    pub username: Username,
    /// Name shown to others.
    pub display_name: DisplayName,
    /// Password.
    pub password: Password,
}

/// `POST /api/login`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginRequest {
    /// Login name (case-insensitive).
    pub username: Username,
    /// Password.
    pub password: Password,
}

/// Successful register or login.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthResponse {
    /// New session token; send it in `Hello` and as `Authorization: Bearer`.
    pub token: SessionToken,
    /// The account.
    pub user: User,
}

/// Failed register or login.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthError {
    /// Registration: the name is taken.
    UsernameTaken,
    /// Login: unknown user or wrong password (deliberately not told apart).
    InvalidCredentials,
    /// Too many attempts; try again later.
    RateLimited,
    /// The client speaks another protocol version.
    UpdateRequired {
        /// The server's protocol version.
        server_version: u16,
    },
}

/// Successful `POST /api/images`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageUploaded {
    /// Content hash, to reference in a command and in `/img/{hash}/…` URLs.
    pub hash: ImageHash,
}

/// Failed `POST /api/images`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageError {
    /// Body larger than [`MAX_IMAGE_BYTES`].
    TooLarge,
    /// More than [`MAX_IMAGE_PIXELS`] pixels.
    TooManyPixels,
    /// Not a JPEG, PNG or WebP, or corrupt.
    Unsupported,
    /// Missing or invalid session.
    Unauthorized,
    /// Too many uploads; try again later.
    RateLimited,
}

/// Upload size limit (SPEC §2, VPS constraints).
pub const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;

/// Upload pixel limit, checked from the header before decoding.
pub const MAX_IMAGE_PIXELS: u64 = 40_000_000;
