//! Beholder server: auth, campaigns, event log, sync, presence and images.

pub mod auth;
pub mod campaigns;
pub mod db;
pub mod http;
pub mod hub;
pub mod images;
pub mod permissions;
pub mod rate_limit;
pub mod ws;

use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::extract::{DefaultBodyLimit, State, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::{get, post};
use beholder_core::{UnixMillis, UserId};
use beholder_protocol::codec::MAX_FRAME_BYTES;
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

use crate::db::Db;
use crate::hub::Hub;
use crate::rate_limit::RateLimiter;

/// Shared server state.
pub type AppState = Arc<Server>;

/// Everything the handlers share.
pub struct Server {
    /// The database.
    pub db: Db,
    /// Live connections.
    pub hub: Hub,
    /// Root for the database file and stored images.
    pub data_dir: PathBuf,
    /// Concurrent Argon2 computations (19 MiB each).
    pub hashing: Semaphore,
    /// Concurrent image upload bodies being buffered (up to 20 MB each).
    pub uploads: Semaphore,
    /// Concurrent image decodes (up to ~150 MB each).
    pub encoding: Semaphore,
    /// Login attempts per client IP.
    pub login_ip_limit: RateLimiter<IpAddr>,
    /// Login attempts per username.
    pub login_user_limit: RateLimiter<String>,
    /// Registrations per client IP.
    pub register_limit: RateLimiter<IpAddr>,
    /// Join attempts per user (invite codes are only 40 bits).
    pub join_limit: RateLimiter<UserId>,
    /// Image uploads per user.
    pub upload_limit: RateLimiter<UserId>,
}

impl Server {
    /// Opens the database under `data_dir` and builds the shared state.
    pub fn open(data_dir: PathBuf) -> Result<AppState, db::DbError> {
        let db = Db::open(&data_dir.join("beholder.db"))?;
        Ok(Self::with_db(db, data_dir))
    }

    /// State around an existing database (tests use an in-memory one).
    pub fn with_db(db: Db, data_dir: PathBuf) -> AppState {
        let minute = Duration::from_secs(60);
        Arc::new(Self {
            db,
            hub: Hub::default(),
            data_dir,
            hashing: Semaphore::new(2),
            uploads: Semaphore::new(2),
            encoding: Semaphore::new(1),
            login_ip_limit: RateLimiter::new(20, minute),
            login_user_limit: RateLimiter::new(10, minute),
            register_limit: RateLimiter::new(5, Duration::from_secs(3600)),
            join_limit: RateLimiter::new(10, minute),
            upload_limit: RateLimiter::new(20, Duration::from_secs(3600)),
        })
    }
}

/// All routes.
pub fn router(state: AppState) -> Router {
    // Auth bodies are tiny; refuse anything bigger before buffering it.
    let small = DefaultBodyLimit::max(4096);
    Router::new()
        .route("/ws", get(ws_upgrade))
        .route("/api/register", post(http::register).layer(small))
        .route("/api/login", post(http::login).layer(small))
        .route("/api/logout", post(http::logout).layer(small))
        .route("/api/images", post(http::upload_image))
        .route("/img/{hash}/{file}", get(http::get_image))
        .with_state(state)
}

async fn ws_upgrade(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.max_message_size(MAX_FRAME_BYTES)
        .max_frame_size(MAX_FRAME_BYTES)
        .on_upgrade(move |socket| ws::run(socket, state))
}

/// Serves until `shutdown` resolves, with daily maintenance in the background.
pub async fn serve(
    listener: TcpListener,
    state: AppState,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    tokio::spawn(maintenance(Arc::clone(&state)));
    axum::serve(
        listener,
        router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
}

/// Compacts event logs and purges stale commands and sessions, once a day.
async fn maintenance(state: AppState) {
    let mut every = tokio::time::interval(Duration::from_secs(24 * 3600));
    loop {
        every.tick().await;
        let r = state
            .db
            .call(|c| {
                campaigns::compact(c, now())?;
                auth::purge_sessions(c, now())
            })
            .await;
        if let Err(e) = r {
            tracing::warn!("maintenance failed: {e}");
        }
    }
}

/// The current time.
pub fn now() -> UnixMillis {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    UnixMillis(i64::try_from(ms).unwrap_or(i64::MAX))
}

/// Bytes from the OS random source.
pub fn random_bytes<const N: usize>() -> Result<[u8; N], db::DbError> {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).map_err(|_| db::DbError::Corrupt("random source failed"))?;
    Ok(b)
}
