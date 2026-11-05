// Returning a ready `Response` as the error is the axum idiom; boxing only adds an allocation.
#![allow(clippy::result_large_err)]

use std::net::{IpAddr, SocketAddr};

use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use beholder_core::ImageHash;
use beholder_core::User;
use beholder_protocol::http::{
    AuthError, AuthResponse, ImageError, ImageUploaded, LoginRequest, MAX_IMAGE_BYTES,
    RegisterRequest, SourceInfo,
};
use beholder_protocol::{PROTOCOL_HEADER, PROTOCOL_VERSION, SessionToken};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::auth::{self, AuthError as InternalAuthError};
use crate::compendium;
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

// https://www.cloudflare.com/ips/ (fetched 2026-11-05).
const CLOUDFLARE_V4: [([u8; 4], u8); 15] = [
    ([173, 245, 48, 0], 20),
    ([103, 21, 244, 0], 22),
    ([103, 22, 200, 0], 22),
    ([103, 31, 4, 0], 22),
    ([141, 101, 64, 0], 18),
    ([108, 162, 192, 0], 18),
    ([190, 93, 240, 0], 20),
    ([188, 114, 96, 0], 20),
    ([197, 234, 240, 0], 22),
    ([198, 41, 128, 0], 17),
    ([162, 158, 0, 0], 15),
    ([104, 16, 0, 0], 13),
    ([104, 24, 0, 0], 14),
    ([172, 64, 0, 0], 13),
    ([131, 0, 72, 0], 22),
];

const CLOUDFLARE_V6: [([u16; 8], u8); 7] = [
    ([0x2400, 0xcb00, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0], 32),
    ([0x2606, 0x4700, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0], 32),
    ([0x2803, 0xf800, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0], 32),
    ([0x2405, 0xb500, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0], 32),
    ([0x2405, 0x8100, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0], 32),
    ([0x2a06, 0x98c0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0], 29),
    ([0x2c0f, 0xf248, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0], 32),
];

fn cloudflare(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => CLOUDFLARE_V4.iter().any(|(net, len)| {
            let mask = u32::MAX.checked_shl(32 - u32::from(*len)).unwrap_or(0);
            u32::from(ip) & mask == u32::from_be_bytes(*net) & mask
        }),
        IpAddr::V6(ip) => CLOUDFLARE_V6.iter().any(|(net, len)| {
            let mask = u128::MAX.checked_shl(128 - u32::from(*len)).unwrap_or(0);
            u128::from(ip) & mask == u128::from(std::net::Ipv6Addr::from(*net)) & mask
        }),
    }
}

// Rate limits key on this: CF-Connecting-IP counts only when Cloudflare is the hop the proxy saw.
fn client_ip(headers: &HeaderMap, peer: SocketAddr) -> IpAddr {
    let hop = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit(',').next())
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(peer.ip());
    if !cloudflare(hop) {
        return hop;
    }
    headers
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(hop)
}

fn bearer(headers: &HeaderMap) -> Option<SessionToken> {
    let v = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    SessionToken::from_hex(v.strip_prefix("Bearer ")?)
}

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

async fn signed_in(state: &AppState, headers: &HeaderMap) -> Result<User, Response> {
    check_version(headers)?;
    let token = bearer(headers).ok_or_else(|| StatusCode::UNAUTHORIZED.into_response())?;
    match state
        .db
        .call(move |c| auth::validate_session(c, &token, now(), false))
        .await
    {
        Ok(Some(s)) => Ok(s.user),
        Ok(None) => Err(StatusCode::UNAUTHORIZED.into_response()),
        Err(e) => Err(internal(e)),
    }
}

async fn library_bundle(state: &AppState) -> Result<SourceInfo, Response> {
    let head = state
        .db
        .call(|c| crate::library::head(c))
        .await
        .map_err(internal)?;
    let mut cache = state.library_cache.lock().await;
    if let Some(c) = cache.as_ref()
        && c.seq <= head
        && head - c.seq <= crate::library::MAX_REPLAY
        && c.path.exists()
    {
        return Ok(c.info.clone());
    }
    let dir = state.data_dir.join("library");
    tokio::fs::create_dir_all(&dir).await.map_err(internal)?;
    let path = dir.join(format!("library-{head}.bundle"));
    let target = path.clone();
    let written = state
        .db
        .call(move |c| crate::library::write_bundle(c, &target))
        .await
        .map_err(internal)?;
    let meta = crate::library::source_meta();
    let info = SourceInfo {
        id: meta.id,
        name: meta.name,
        publisher: meta.publisher,
        system: meta.system,
        rules: meta.rules,
        sha256: written.sha256,
        bytes: written.bytes,
    };
    if let Some(old) = cache.take()
        && old.path != path
    {
        let _ = tokio::fs::remove_file(&old.path).await;
    }
    *cache = Some(compendium::CachedBundle {
        info: info.clone(),
        seq: written.seq,
        path,
    });
    Ok(info)
}

pub async fn compendium_index(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(r) = signed_in(&state, &headers).await {
        return r;
    }
    let mut index = state.compendium.index();
    match library_bundle(&state).await {
        Ok(info) => index.insert(0, info),
        Err(r) => return r,
    }
    reply(StatusCode::OK, &index)
}

pub async fn encounter_as(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((campaign, encounter, player)): Path<(
        beholder_core::CampaignId,
        beholder_core::EncounterId,
        beholder_core::UserId,
    )>,
) -> Response {
    let me = match signed_in(&state, &headers).await {
        Ok(u) => u.id,
        Err(r) => return r,
    };
    match state
        .db
        .call(move |c| crate::combat::encounter_as(c, campaign, me, encounter, player))
        .await
    {
        Ok(crate::combat::AsPlayer::View(e)) => reply(StatusCode::OK, &e),
        Ok(crate::combat::AsPlayer::Forbidden) => StatusCode::FORBIDDEN.into_response(),
        Ok(crate::combat::AsPlayer::NotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => internal(e),
    }
}

pub async fn library_changes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(after): Path<u64>,
) -> Response {
    if let Err(r) = signed_in(&state, &headers).await {
        return r;
    }
    match state
        .db
        .call(move |c| crate::library::changes(c, after))
        .await
    {
        Ok(changes) => reply(StatusCode::OK, &changes),
        Err(e) => internal(e),
    }
}

pub async fn library_imports(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(r) = signed_in(&state, &headers).await {
        return r;
    }
    match state.db.call(|c| crate::library::imports(c)).await {
        Ok(list) => reply(StatusCode::OK, &list),
        Err(e) => internal(e),
    }
}

pub async fn compendium_bundle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sha256): Path<String>,
) -> Response {
    if let Err(r) = signed_in(&state, &headers).await {
        return r;
    }
    let library_path = state
        .library_cache
        .lock()
        .await
        .as_ref()
        .filter(|c| c.info.sha256 == sha256)
        .map(|c| c.path.clone());
    let bytes = match library_path {
        Some(p) => match tokio::fs::read(&p).await {
            Ok(b) => Bytes::from(b),
            Err(_) => return StatusCode::NOT_FOUND.into_response(),
        },
        None => match state.compendium.bundle(&sha256) {
            Some(b) => b,
            None => return StatusCode::NOT_FOUND.into_response(),
        },
    };
    let len = bytes.len();
    let range = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
    let (status, body, content_range) = match range {
        None => (StatusCode::OK, bytes, None),
        Some(r) => match compendium::parse_range(r, len) {
            Some(start) => (
                StatusCode::PARTIAL_CONTENT,
                bytes.slice(start..),
                Some(format!("bytes {start}-{}/{len}", len - 1)),
            ),
            None => {
                let mut resp = StatusCode::RANGE_NOT_SATISFIABLE.into_response();
                if let Ok(v) = HeaderValue::from_str(&format!("bytes */{len}")) {
                    resp.headers_mut().insert(header::CONTENT_RANGE, v);
                }
                return resp;
            }
        },
    };
    let mut resp = (status, body).into_response();
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=31536000, immutable"),
    );
    if let Some(v) = content_range.and_then(|v| HeaderValue::from_str(&v).ok()) {
        h.insert(header::CONTENT_RANGE, v);
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_client_ips_behind_cloudflare_and_a_proxy() {
        let peer: SocketAddr = "10.244.0.7:40000".parse().unwrap();
        let headers = |pairs: &[(&'static str, &'static str)]| {
            let mut h = HeaderMap::new();
            for (k, v) in pairs {
                h.insert(*k, HeaderValue::from_static(v));
            }
            h
        };
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        assert_eq!(client_ip(&headers(&[]), peer), ip("10.244.0.7"));
        assert_eq!(
            client_ip(&headers(&[("x-forwarded-for", "203.0.113.9")]), peer),
            ip("203.0.113.9"),
            "a direct client behind the proxy"
        );
        assert_eq!(
            client_ip(
                &headers(&[
                    ("x-forwarded-for", "172.68.10.20"),
                    ("cf-connecting-ip", "198.51.100.4")
                ]),
                peer
            ),
            ip("198.51.100.4"),
            "through a Cloudflare edge, the player's own address"
        );
        assert_eq!(
            client_ip(
                &headers(&[
                    ("x-forwarded-for", "2606:4700:3030::ac43:9b6c"),
                    ("cf-connecting-ip", "2001:db8::1")
                ]),
                peer
            ),
            ip("2001:db8::1")
        );
        assert_eq!(
            client_ip(
                &headers(&[
                    ("x-forwarded-for", "203.0.113.9"),
                    ("cf-connecting-ip", "198.51.100.4")
                ]),
                peer
            ),
            ip("203.0.113.9"),
            "a forged header from outside Cloudflare is ignored"
        );
        assert!(cloudflare(ip("104.16.0.1")) && cloudflare(ip("131.0.75.255")));
        assert!(!cloudflare(ip("131.0.76.0")) && !cloudflare(ip("45.88.180.140")));
    }
}
