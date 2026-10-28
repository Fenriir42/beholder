pub mod auth;
pub mod campaigns;
pub mod characters;
pub mod compendium;
pub mod db;
pub mod http;
pub mod hub;
pub mod images;
pub mod library;
pub mod notes;
pub mod permissions;
pub mod rate_limit;
pub mod setup_import;
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

use crate::compendium::Library;
use crate::db::Db;
use crate::hub::Hub;
use crate::rate_limit::RateLimiter;

pub type AppState = Arc<Server>;

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("database: {0}")]
    Db(#[from] db::DbError),
    #[error("compendium: {0}")]
    Bundle(#[from] compendium::BundleError),
}

pub struct Server {
    pub db: Db,
    pub compendium: Library,
    pub library_cache: tokio::sync::Mutex<Option<compendium::CachedBundle>>,
    pub hub: Hub,
    pub data_dir: PathBuf,
    pub hashing: Semaphore,
    pub uploads: Semaphore,
    pub encoding: Semaphore,
    pub login_ip_limit: RateLimiter<IpAddr>,
    pub login_user_limit: RateLimiter<String>,
    pub register_limit: RateLimiter<IpAddr>,
    pub join_limit: RateLimiter<UserId>,
    pub upload_limit: RateLimiter<UserId>,
}

impl Server {
    pub fn open(data_dir: PathBuf) -> Result<AppState, StartError> {
        let db = Db::open(&data_dir.join("beholder.db"))?;
        Ok(Self::with_db(db, data_dir, Library::builtin()?))
    }

    pub fn with_db(db: Db, data_dir: PathBuf, compendium: Library) -> AppState {
        let minute = Duration::from_secs(60);
        Arc::new(Self {
            db,
            compendium,
            library_cache: tokio::sync::Mutex::new(None),
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

pub fn router(state: AppState) -> Router {
    let small = DefaultBodyLimit::max(4096);
    Router::new()
        .route("/ws", get(ws_upgrade))
        .route("/api/register", post(http::register).layer(small))
        .route("/api/login", post(http::login).layer(small))
        .route("/api/logout", post(http::logout).layer(small))
        .route("/api/images", post(http::upload_image))
        .route("/img/{hash}/{file}", get(http::get_image))
        .route("/api/compendium", get(http::compendium_index))
        .route("/api/compendium/{sha256}", get(http::compendium_bundle))
        .route("/api/library/changes/{after}", get(http::library_changes))
        .route("/api/library/imports", get(http::library_imports))
        .with_state(state)
}

async fn ws_upgrade(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.max_message_size(MAX_FRAME_BYTES)
        .max_frame_size(MAX_FRAME_BYTES)
        .on_upgrade(move |socket| ws::run(socket, state))
}

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

pub fn now() -> UnixMillis {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    UnixMillis(i64::try_from(ms).unwrap_or(i64::MAX))
}

pub fn random_bytes<const N: usize>() -> Result<[u8; N], db::DbError> {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).map_err(|_| db::DbError::Corrupt("random source failed"))?;
    Ok(b)
}
