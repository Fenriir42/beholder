//! HTTPS endpoints: auth, image upload and image download. Bodies are postcard.

// Early-returning a ready `Response` as the error is the axum idiom here; boxing
// it would only add an allocation on the error path.
#![allow(clippy::result_large_err)]

use std::net::{IpAddr, SocketAddr};

use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use beholder_core::ImageHash;
use beholder_protocol::http::{
    AuthError, AuthResponse, ImageError, ImageUploaded, LoginRequest, MAX_IMAGE_BYTES,
    RegisterRequest,
};
use beholder_protocol::{PROTOCOL_HEADER, PROTOCOL_VERSION, SessionToken};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::auth::{self, AuthError as InternalAuthError};
use crate::images::{self, Variant};
use crate::{AppState, now};

const POSTCARD: &str = "application/x-postcard";

fn reply<T: Serialize>(status: StatusCode, body: &T) -> Response {
    match postcard::to_stdvec(body) {
        Ok(b) => (status, [(header::CONTENT_TYPE, POSTCARD)], b).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn internal(e: impl std::fmt::Display) -> Response {
    tracing::error!("{e}");
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

/// `426 Upgrade Required` unless the client speaks our protocol version.
fn check_version(headers: &HeaderMap) -> Result<(), Response> {
    let v = headers
        .get(PROTOCOL_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u16>().ok());
    if v == Some(PROTOCOL_VERSION) {
        return Ok(());
    }
    Err(reply(
        StatusCode::UPGRADE_REQUIRED,
        &AuthError::UpdateRequired {
            server_version: PROTOCOL_VERSION,
        },
    ))
}

fn decode<T: DeserializeOwned>(body: &[u8]) -> Result<T, Response> {
    postcard::from_bytes(body).map_err(|_| StatusCode::BAD_REQUEST.into_response())
}

/// The client address. Behind Caddy, the last `X-Forwarded-For` hop is the
/// one Caddy itself appended, so it can't be spoofed by the client.
fn client_ip(headers: &HeaderMap, peer: SocketAddr) -> IpAddr {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit(',').next())
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(peer.ip())
}

fn bearer(headers: &HeaderMap) -> Option<SessionToken> {
    let v = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    SessionToken::from_hex(v.strip_prefix("Bearer ")?)
}

/// Argon2 on the blocking pool, at most two at a time to bound memory.
async fn run_hashing<T: Send + 'static>(
    state: &AppState,
    f: impl FnOnce() -> Result<T, InternalAuthError> + Send + 'static,
) -> Result<T, Response> {
    let _permit = state.hashing.acquire().await.map_err(internal)?;
    tokio::task::spawn_blocking(f)
        .await
        .map_err(internal)?
        .map_err(internal)
}

/// `POST /api/register`.
pub async fn register(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(r) = check_version(&headers) {
        return r;
    }
    if !state.register_limit.allow(client_ip(&headers, peer)) {
        return reply(StatusCode::TOO_MANY_REQUESTS, &AuthError::RateLimited);
    }
    let req: RegisterRequest = match decode(&body) {
        Ok(r) => r,
        Err(r) => return r,
    };
    let password = req.password.clone();
    let phc = match run_hashing(&state, move || auth::hash_password(&password)).await {
        Ok(p) => p,
        Err(r) => return r,
    };
    let created = state
        .db
        .call(move |c| {
            let Some(user) = auth::create_user(c, &req.username, &req.display_name, &phc, now())?
            else {
                return Ok(None);
            };
            let token = auth::create_session(c, user.id, now())?;
            Ok::<_, InternalAuthError>(Some(AuthResponse { token, user }))
        })
        .await;
    match created {
        Ok(Some(resp)) => reply(StatusCode::OK, &resp),
        Ok(None) => reply(StatusCode::CONFLICT, &AuthError::UsernameTaken),
        Err(e) => internal(e),
    }
}

/// `POST /api/login`.
pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(r) = check_version(&headers) {
        return r;
    }
    let req: LoginRequest = match decode(&body) {
        Ok(r) => r,
        Err(r) => return r,
    };
    let name_key = req.username.as_str().to_ascii_lowercase();
    if !state.login_ip_limit.allow(client_ip(&headers, peer))
        || !state.login_user_limit.allow(name_key)
    {
        return reply(StatusCode::TOO_MANY_REQUESTS, &AuthError::RateLimited);
    }
    let username = req.username.clone();
    let found = match state
        .db
        .call(move |c| auth::credentials(c, &username))
        .await
    {
        Ok(f) => f,
        Err(e) => return internal(e),
    };
    let (user, phc) = found.map_or((None, None), |(u, p)| (Some(u), Some(p)));
    let password = req.password;
    let ok = run_hashing(&state, move || {
        auth::verify_password(&password, phc.as_deref())
    })
    .await;
    let user = match (ok, user) {
        (Ok(true), Some(u)) => u,
        (Ok(_), _) => return reply(StatusCode::UNAUTHORIZED, &AuthError::InvalidCredentials),
        (Err(r), _) => return r,
    };
    let id = user.id;
    match state
        .db
        .call(move |c| auth::create_session(c, id, now()))
        .await
    {
        Ok(token) => reply(StatusCode::OK, &AuthResponse { token, user }),
        Err(e) => internal(e),
    }
}

/// `POST /api/logout` with `Authorization: Bearer <token>`.
pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(token) = bearer(&headers) else {
        return StatusCode::NO_CONTENT.into_response();
    };
    match state
        .db
        .call(move |c| auth::delete_session(c, &token))
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => internal(e),
    }
}

/// `POST /api/images`: raw image bytes, `Authorization: Bearer <token>`.
pub async fn upload_image(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    if let Err(r) = check_version(&headers) {
        return r;
    }
    let Some(token) = bearer(&headers) else {
        return reply(StatusCode::UNAUTHORIZED, &ImageError::Unauthorized);
    };
    let session = state
        .db
        .call(move |c| auth::validate_session(c, &token, now(), false))
        .await;
    let user = match session {
        Ok(Some(s)) => s.user,
        Ok(None) => return reply(StatusCode::UNAUTHORIZED, &ImageError::Unauthorized),
        Err(e) => return internal(e),
    };
    if !state.upload_limit.allow(user.id) {
        return reply(StatusCode::TOO_MANY_REQUESTS, &ImageError::RateLimited);
    }
    // Bounds how many 20 MB bodies are buffered at once.
    let Ok(_body_permit) = state.uploads.acquire().await else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Ok(bytes) = axum::body::to_bytes(body, MAX_IMAGE_BYTES).await else {
        return reply(StatusCode::PAYLOAD_TOO_LARGE, &ImageError::TooLarge);
    };
    let hash = images::hash(&bytes);
    match state.db.call(move |c| images::exists(c, &hash)).await {
        Ok(true) => return reply(StatusCode::OK, &ImageUploaded { hash }),
        Ok(false) => {}
        Err(e) => return internal(e),
    }
    // One decode/encode at a time: a full-size decode can take 150 MB.
    let Ok(_encode_permit) = state.encoding.acquire().await else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let root = state.data_dir.clone();
    let encoded = tokio::task::spawn_blocking(move || {
        let enc = images::process(&bytes)?;
        drop(bytes);
        images::store_files(&root, &hash, &enc).map_err(|e| {
            tracing::error!("storing image: {e}");
            ImageError::Unsupported
        })?;
        Ok::<_, ImageError>(enc)
    })
    .await;
    let enc = match encoded {
        Ok(Ok(enc)) => enc,
        Ok(Err(e)) => {
            let status = match e {
                ImageError::TooManyPixels | ImageError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
                _ => StatusCode::UNPROCESSABLE_ENTITY,
            };
            return reply(status, &e);
        }
        Err(e) => return internal(e),
    };
    let uploader = user.id;
    match state
        .db
        .call(move |c| images::record(c, &hash, &enc, uploader, now()))
        .await
    {
        Ok(()) => reply(StatusCode::OK, &ImageUploaded { hash }),
        Err(e) => internal(e),
    }
}

/// `GET /img/{hash}/{thumb|display}.webp`, immutable and cacheable forever.
pub async fn get_image(
    State(state): State<AppState>,
    Path((hash, file)): Path<(String, String)>,
) -> Response {
    let (Some(hash), Some(variant)) = (ImageHash::from_hex(&hash), Variant::from_file_name(&file))
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match tokio::fs::read(images::path(&state.data_dir, &hash, variant)).await {
        Ok(bytes) => {
            let mut resp = bytes.into_response();
            let h = resp.headers_mut();
            h.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/webp"));
            h.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            );
            resp
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
