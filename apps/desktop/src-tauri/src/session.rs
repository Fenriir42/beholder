use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use beholder_client_core::cache::Cache;
use beholder_client_core::http::{Http, HttpError};
use beholder_client_core::{Client, ClientEvent, CompendiumState, ConnectionState};
use beholder_core::UserId;
use beholder_ipc::{
    CampaignView, CompendiumStatus, ConnState, IpcError, Rejected, ViewData, events,
};
use beholder_protocol::SessionToken;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::config::Config;
use crate::secrets::Secrets;

pub struct Shell {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub image_dir: PathBuf,
    pub config: Mutex<Config>,
    pub secrets: Secrets,
    pub session: Mutex<Option<Session>>,
}

#[derive(Clone)]
pub struct Session {
    pub server_url: String,
    pub client: Client,
    pub token: std::sync::Arc<Mutex<SessionToken>>,
}

pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Shell {
    pub fn http(&self) -> Result<(String, Http), IpcError> {
        let url = lock(&self.config)
            .server_url
            .clone()
            .ok_or(IpcError::BadServerUrl)?;
        let http = Http::new(&url);
        Ok((url, http))
    }

    pub fn session(&self) -> Result<Session, IpcError> {
        lock(&self.session).clone().ok_or(IpcError::SignedOut)
    }

    pub fn save_config(&self) -> Result<(), IpcError> {
        lock(&self.config)
            .save(&self.config_dir)
            .map_err(|_| IpcError::Storage)
    }

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

    pub fn stop(&self) {
        if let Some(s) = lock(&self.session).take() {
            s.client.shutdown();
        }
    }
}

fn cache_file(server_url: &str, user: UserId) -> String {
    let server: String = server_url
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{server}-{user}.sqlite")
}

pub fn conn_state(s: ConnectionState) -> ConnState {
    match s {
        ConnectionState::Connecting => ConnState::Connecting,
        ConnectionState::Online => ConnState::Online,
        ConnectionState::Reconnecting { queued } => ConnState::Reconnecting { queued },
        ConnectionState::Offline { queued } => ConnState::Offline { queued },
    }
}

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
                pending_notes: c.pending_notes,
                online: c.online,
            })
            .collect(),
        pending_joins: v.pending_joins,
        pending_library: v.pending_library,
    })
}

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
            ClientEvent::Compendium(s) => app.emit(
                events::COMPENDIUM,
                match s {
                    CompendiumState::Syncing => CompendiumStatus::Syncing,
                    CompendiumState::Ready => CompendiumStatus::Ready,
                    CompendiumState::Failed => CompendiumStatus::Failed,
                },
            ),
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
