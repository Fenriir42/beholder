//! Shell state and the signed-in session: owns the client-core `Client` and
//! forwards its events to the UI.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use beholder_client_core::cache::Cache;
use beholder_client_core::http::{Http, HttpError};
use beholder_client_core::{Client, ClientEvent, ConnectionState};
use beholder_core::UserId;
use beholder_ipc::{CampaignView, ConnState, IpcError, Rejected, ViewData, events};
use beholder_protocol::SessionToken;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::config::Config;
use crate::secrets::Secrets;

/// Everything the commands share.
pub struct Shell {
    /// Where `config.json` lives.
    pub config_dir: PathBuf,
    /// Where caches live (per server and user).
    pub data_dir: PathBuf,
    /// Where downloaded images are cached by hash.
    pub image_dir: PathBuf,
    /// Remembered settings.
    pub config: Mutex<Config>,
    /// OS keyring.
    pub secrets: Secrets,
    /// The signed-in session, if any.
    pub session: Mutex<Option<Session>>,
}

/// A signed-in session.
#[derive(Clone)]
pub struct Session {
    /// The server this session belongs to.
    pub server_url: String,
    /// The running client.
    pub client: Client,
    /// Current token (rotated by the server from time to time).
    pub token: std::sync::Arc<Mutex<SessionToken>>,
}

pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Shell {
    /// The configured server's HTTP client.
    pub fn http(&self) -> Result<(String, Http), IpcError> {
        let url = lock(&self.config)
            .server_url
            .clone()
            .ok_or(IpcError::BadServerUrl)?;
        let http = Http::new(&url);
        Ok((url, http))
    }

    /// The running session.
    pub fn session(&self) -> Result<Session, IpcError> {
        lock(&self.session).clone().ok_or(IpcError::SignedOut)
    }

    /// Persists the config.
    pub fn save_config(&self) -> Result<(), IpcError> {
        lock(&self.config)
            .save(&self.config_dir)
            .map_err(|_| IpcError::Storage)
    }

    /// Opens the user's cache and starts the client. Must run inside the
    /// async runtime (the client spawns its connection task).
    pub fn start(
        &self,
        app: &AppHandle,
        server_url: String,
        user: UserId,
        token: SessionToken,
    ) -> Result<(), IpcError> {
        self.stop();
        std::fs::create_dir_all(&self.data_dir).map_err(|_| IpcError::Storage)?;
        let cache = Cache::open(&self.data_dir.join(cache_file(&server_url, user)))
            .map_err(|_| IpcError::Storage)?;
        let (client, rx) = Client::start(&server_url, cache, token.clone());
        let session = Session {
            server_url: server_url.clone(),
            client,
            token: std::sync::Arc::new(Mutex::new(token)),
        };
        tauri::async_runtime::spawn(forward(app.clone(), rx, session.clone()));
        *lock(&self.session) = Some(session);
        Ok(())
    }

    /// Stops the client, keeping its cache on disk.
    pub fn stop(&self) {
        if let Some(s) = lock(&self.session).take() {
            s.client.shutdown();
        }
    }
}

/// One cache file per (server, user), so accounts never see each other's data.
fn cache_file(server_url: &str, user: UserId) -> String {
    let server: String = server_url
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{server}-{user}.sqlite")
}

/// The connection state as the UI sees it.
pub fn conn_state(s: ConnectionState) -> ConnState {
    match s {
        ConnectionState::Connecting => ConnState::Connecting,
        ConnectionState::Online => ConnState::Online,
        ConnectionState::Reconnecting { queued } => ConnState::Reconnecting { queued },
        ConnectionState::Offline { queued } => ConnState::Offline { queued },
    }
}

/// The current view as the UI sees it.
pub fn view_data(client: &Client) -> Result<ViewData, IpcError> {
    let v = client.view().map_err(|_| IpcError::Storage)?;
    Ok(ViewData {
        user: v.user,
        campaigns: v
            .campaigns
            .into_iter()
            .map(|c| CampaignView {
                campaign: c.campaign,
                pending: c.pending,
                online: c.online,
            })
            .collect(),
        pending_joins: v.pending_joins,
    })
}

/// Maps an HTTP failure to what the UI shows.
pub fn http_error(e: HttpError) -> IpcError {
    match e {
        HttpError::Auth(a) => IpcError::Auth(a),
        HttpError::Image(i) => IpcError::Image(i),
        HttpError::Network(_) => IpcError::Network,
        HttpError::Status(_) | HttpError::Decode => IpcError::Server,
    }
}

async fn forward(app: AppHandle, mut rx: UnboundedReceiver<ClientEvent>, session: Session) {
    while let Some(event) = rx.recv().await {
        // Emit failures only mean the window is gone; nothing to do.
        let _ = match event {
            ClientEvent::State(s) => app.emit(events::STATE, conn_state(s)),
            ClientEvent::ViewChanged => app.emit(events::VIEW_CHANGED, ()),
            ClientEvent::Rejected { reason, .. } => app.emit(events::REJECTED, Rejected { reason }),
            ClientEvent::SessionRotated(token) => {
                *lock(&session.token) = token.clone();
                app.state::<Shell>()
                    .secrets
                    .save(&session.server_url, &token);
                Ok(())
            }
            ClientEvent::SessionInvalid => {
                let shell = app.state::<Shell>();
                shell.secrets.delete(&session.server_url);
                shell.stop();
                app.emit(events::SIGNED_OUT, ())
            }
            ClientEvent::UpdateRequired { server_version } => {
                app.emit(events::UPDATE_REQUIRED, server_version)
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_files_are_per_server_and_user() {
        let u = UserId::from_bytes([0; 16]);
        assert_eq!(
            cache_file("https://dnd.example.org:8443", u),
            "https___dnd_example_org_8443-00000000-0000-0000-0000-000000000000.sqlite"
        );
    }
}
