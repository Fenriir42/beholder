//! Tauri commands called by the UI. Each user action becomes a client-core
//! command (outbox first, then the network); auth and uploads use HTTPS.

use beholder_core::{
    CampaignId, CampaignOp, CampaignSettings, CampaignTitle, DisplayName, InviteCode, Password,
    Username,
};
use beholder_ipc::{Boot, ConnState, IpcError, ViewData};
use beholder_protocol::CommandKind;
use beholder_protocol::http::{AuthResponse, LoginRequest, RegisterRequest};
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, State};

use crate::config::normalize_server_url;
use crate::session::{Shell, conn_state, http_error, lock, view_data};

/// Base URL of the `img` protocol: Windows serves custom schemes over http.
const IMAGE_BASE: &str = if cfg!(windows) {
    "http://img.localhost/"
} else {
    "img://localhost/"
};

/// Startup info for the UI.
#[tauri::command(rename_all = "snake_case")]
pub fn boot(shell: State<'_, Shell>) -> Boot {
    let config = lock(&shell.config).clone();
    Boot {
        server_url: config.server_url,
        signed_in: lock(&shell.session).is_some(),
        locale: config.locale,
        image_base: IMAGE_BASE.to_owned(),
    }
}

/// Current connection state.
#[tauri::command(rename_all = "snake_case")]
pub fn connection_state(shell: State<'_, Shell>) -> ConnState {
    lock(&shell.session)
        .as_ref()
        .map_or(ConnState::Connecting, |s| conn_state(s.client.state()))
}

/// Remembers the server; switching servers ends the current session.
#[tauri::command(rename_all = "snake_case")]
pub fn set_server_url(shell: State<'_, Shell>, url: String) -> Result<String, IpcError> {
    let url = normalize_server_url(&url).ok_or(IpcError::BadServerUrl)?;
    let changed = lock(&shell.config).server_url.as_deref() != Some(url.as_str());
    if changed {
        shell.stop();
        let mut c = lock(&shell.config);
        c.server_url = Some(url.clone());
        c.last_user = None;
    }
    shell.save_config()?;
    Ok(url)
}

/// Remembers the UI language.
#[tauri::command(rename_all = "snake_case")]
pub fn set_locale(shell: State<'_, Shell>, locale: String) -> Result<(), IpcError> {
    lock(&shell.config).locale = Some(locale);
    shell.save_config()
}

async fn sign_in(
    app: &AppHandle,
    shell: &Shell,
    call: impl FnOnce(
        &beholder_client_core::http::Http,
    ) -> Result<AuthResponse, beholder_client_core::http::HttpError>
    + Send
    + 'static,
) -> Result<(), IpcError> {
    let (url, http) = shell.http()?;
    let resp = tauri::async_runtime::spawn_blocking(move || call(&http))
        .await
        .map_err(|_| IpcError::Server)?
        .map_err(http_error)?;
    // Without a keyring the session still works; it just won't survive a restart.
    shell.secrets.save(&url, &resp.token);
    lock(&shell.config).last_user = Some(resp.user.id);
    shell.save_config()?;
    shell.start(app, url, resp.user.id, resp.token)
}

/// Logs in.
#[tauri::command(rename_all = "snake_case")]
pub async fn login(
    app: AppHandle,
    shell: State<'_, Shell>,
    username: String,
    password: String,
) -> Result<(), IpcError> {
    let req = LoginRequest {
        username: Username::new(username).map_err(IpcError::Invalid)?,
        password: Password::new(password).map_err(IpcError::Invalid)?,
    };
    sign_in(&app, &shell, move |h| h.login(&req)).await
}

/// Creates an account and logs in.
#[tauri::command(rename_all = "snake_case")]
pub async fn register(
    app: AppHandle,
    shell: State<'_, Shell>,
    username: String,
    display_name: String,
    password: String,
) -> Result<(), IpcError> {
    let req = RegisterRequest {
        username: Username::new(username).map_err(IpcError::Invalid)?,
        display_name: DisplayName::new(&display_name).map_err(IpcError::Invalid)?,
        password: Password::new(password).map_err(IpcError::Invalid)?,
    };
    sign_in(&app, &shell, move |h| h.register(&req)).await
}

/// Signs out: ends the server session (best effort) and forgets the token.
#[tauri::command(rename_all = "snake_case")]
pub async fn logout(shell: State<'_, Shell>) -> Result<(), IpcError> {
    let Ok(session) = shell.session() else {
        return Ok(());
    };
    shell.stop();
    shell.secrets.delete(&session.server_url);
    let token = lock(&session.token).clone();
    let http = beholder_client_core::http::Http::new(&session.server_url);
    // Offline sign-out still works locally; the server session then just expires.
    let _ = tauri::async_runtime::spawn_blocking(move || http.logout(&token)).await;
    Ok(())
}

/// The optimistic view of all campaigns.
#[tauri::command(rename_all = "snake_case")]
pub fn view(shell: State<'_, Shell>) -> Result<ViewData, IpcError> {
    view_data(&shell.session()?.client)
}

/// Reconnect now instead of waiting for the backoff.
#[tauri::command(rename_all = "snake_case")]
pub fn retry_now(shell: State<'_, Shell>) {
    if let Ok(s) = shell.session() {
        s.client.network_changed();
    }
}

fn submit(shell: &Shell, kind: CommandKind) -> Result<(), IpcError> {
    shell
        .session()?
        .client
        .submit(kind)
        .map(|_| ())
        .map_err(|_| IpcError::Storage)
}

fn campaign_id(s: &str) -> Result<CampaignId, IpcError> {
    s.parse()
        .map(CampaignId)
        .map_err(|_| IpcError::Invalid(beholder_core::ValidationError::Character))
}

/// Creates a campaign; returns its id (usable before the server answers).
#[tauri::command(rename_all = "snake_case")]
pub fn create_campaign(shell: State<'_, Shell>, title: String) -> Result<String, IpcError> {
    let title = CampaignTitle::new(&title).map_err(IpcError::Invalid)?;
    let id = CampaignId(uuid::Uuid::now_v7());
    submit(
        &shell,
        CommandKind::CreateCampaign {
            campaign_id: id,
            title,
        },
    )?;
    Ok(id.to_string())
}

/// Joins a campaign by invite code.
#[tauri::command(rename_all = "snake_case")]
pub fn join_campaign(shell: State<'_, Shell>, code: String) -> Result<(), IpcError> {
    let code = InviteCode::parse(&code).map_err(IpcError::Invalid)?;
    submit(&shell, CommandKind::JoinCampaign { code })
}

fn campaign_op(shell: &Shell, campaign_id: &str, op: CampaignOp) -> Result<(), IpcError> {
    submit(
        shell,
        CommandKind::Campaign {
            campaign_id: self::campaign_id(campaign_id)?,
            op,
        },
    )
}

/// Renames a campaign (DM).
#[tauri::command(rename_all = "snake_case")]
pub fn rename_campaign(
    shell: State<'_, Shell>,
    campaign_id: String,
    title: String,
) -> Result<(), IpcError> {
    let title = CampaignTitle::new(&title).map_err(IpcError::Invalid)?;
    campaign_op(&shell, &campaign_id, CampaignOp::Rename { title })
}

/// Replaces the campaign settings (DM).
#[tauri::command(rename_all = "snake_case")]
pub fn update_settings(
    shell: State<'_, Shell>,
    campaign_id: String,
    settings: CampaignSettings,
) -> Result<(), IpcError> {
    campaign_op(&shell, &campaign_id, CampaignOp::UpdateSettings(settings))
}

/// Replaces the invite code (DM).
#[tauri::command(rename_all = "snake_case")]
pub fn regenerate_invite(shell: State<'_, Shell>, campaign_id: String) -> Result<(), IpcError> {
    campaign_op(&shell, &campaign_id, CampaignOp::RegenerateInvite)
}

/// Uploads a cover (raw body, `campaign-id` header) and sets it (DM).
/// Needs the network: an image is too big to queue in the outbox.
#[tauri::command(rename_all = "snake_case")]
pub async fn upload_cover(shell: State<'_, Shell>, request: Request<'_>) -> Result<(), IpcError> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(IpcError::Image(
            beholder_protocol::http::ImageError::Unsupported,
        ));
    };
    let id = request
        .headers()
        .get("campaign-id")
        .and_then(|v| v.to_str().ok())
        .ok_or(IpcError::Server)?
        .to_owned();
    let session = shell.session()?;
    let token = lock(&session.token).clone();
    let http = beholder_client_core::http::Http::new(&session.server_url);
    let bytes = bytes.clone();
    let hash = tauri::async_runtime::spawn_blocking(move || http.upload_image(&token, &bytes))
        .await
        .map_err(|_| IpcError::Server)?
        .map_err(http_error)?;
    campaign_op(&shell, &id, CampaignOp::SetCover { cover: Some(hash) })
}
