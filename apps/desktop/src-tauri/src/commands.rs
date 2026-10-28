use beholder_client_core::ClientError;
use beholder_client_core::library_file::FileFormat;
use beholder_core::character::CharacterOp;
use beholder_core::compendium::{FacetOptions, Hit, Kind, RulesVersion, SearchQuery};
use beholder_core::library::{ImportSummary, LibraryDraft};
use beholder_core::mapping::Profile;
use beholder_core::paste::Pasted;
use beholder_core::{
    Audience, CampaignId, CampaignOp, CampaignSettings, CampaignTitle, DisplayName, ImageHash,
    InviteCode, NoteBody, NoteId, NoteOp, NoteTitle, Password, Username,
};
use beholder_core::{ImportId, LibraryId};
use beholder_ipc::{
    Boot, BulkOpened, BulkScanView, ConnState, Credits, EntryView, IpcError, PasteItem,
    ResolveItem, ViewData,
};
use beholder_protocol::CommandKind;
use beholder_protocol::http::{AuthResponse, LoginRequest, RegisterRequest};
use beholder_protocol::http::{ImportInfo, SourceInfo};
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Manager, State};

use crate::config::{load_profiles, normalize_server_url, save_profiles};
use crate::session::{BulkFile, Shell, conn_state, http_error, lock, view_data};

const IMAGE_BASE: &str = if cfg!(windows) {
    "http://img.localhost/"
} else {
    "img://localhost/"
};

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

#[tauri::command(rename_all = "snake_case")]
pub fn connection_state(shell: State<'_, Shell>) -> ConnState {
    lock(&shell.session)
        .as_ref()
        .map_or(ConnState::Connecting, |s| conn_state(s.client.state()))
}

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
    shell.secrets.save(&url, &resp.token);
    lock(&shell.config).last_user = Some(resp.user.id);
    shell.save_config()?;
    shell.start(app, url, resp.user.id, resp.token)
}

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

#[tauri::command(rename_all = "snake_case")]
pub async fn logout(shell: State<'_, Shell>) -> Result<(), IpcError> {
    let Ok(session) = shell.session() else {
        return Ok(());
    };
    shell.stop();
    shell.secrets.delete(&session.server_url);
    let token = lock(&session.token).clone();
    let http = beholder_client_core::http::Http::new(&session.server_url);
    let _ = tauri::async_runtime::spawn_blocking(move || http.logout(&token)).await;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn view(shell: State<'_, Shell>) -> Result<ViewData, IpcError> {
    view_data(&shell.session()?.client)
}

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

#[tauri::command(rename_all = "snake_case")]
pub fn rename_campaign(
    shell: State<'_, Shell>,
    campaign_id: String,
    title: String,
) -> Result<(), IpcError> {
    let title = CampaignTitle::new(&title).map_err(IpcError::Invalid)?;
    campaign_op(&shell, &campaign_id, CampaignOp::Rename { title })
}

#[tauri::command(rename_all = "snake_case")]
pub fn update_settings(
    shell: State<'_, Shell>,
    campaign_id: String,
    settings: CampaignSettings,
) -> Result<(), IpcError> {
    campaign_op(&shell, &campaign_id, CampaignOp::UpdateSettings(settings))
}

#[tauri::command(rename_all = "snake_case")]
pub fn regenerate_invite(shell: State<'_, Shell>, campaign_id: String) -> Result<(), IpcError> {
    campaign_op(&shell, &campaign_id, CampaignOp::RegenerateInvite)
}

async fn upload(shell: &Shell, request: &Request<'_>) -> Result<ImageHash, IpcError> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(IpcError::Image(
            beholder_protocol::http::ImageError::Unsupported,
        ));
    };
    let session = shell.session()?;
    let token = lock(&session.token).clone();
    let http = beholder_client_core::http::Http::new(&session.server_url);
    let bytes = bytes.clone();
    tauri::async_runtime::spawn_blocking(move || http.upload_image(&token, &bytes))
        .await
        .map_err(|_| IpcError::Server)?
        .map_err(http_error)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn upload_cover(shell: State<'_, Shell>, request: Request<'_>) -> Result<(), IpcError> {
    let id = request
        .headers()
        .get("campaign-id")
        .and_then(|v| v.to_str().ok())
        .ok_or(IpcError::Server)?
        .to_owned();
    let hash = upload(&shell, &request).await?;
    campaign_op(&shell, &id, CampaignOp::SetCover { cover: Some(hash) })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn upload_image(
    shell: State<'_, Shell>,
    request: Request<'_>,
) -> Result<String, IpcError> {
    Ok(upload(&shell, &request).await?.to_hex())
}

fn note_id(s: &str) -> Result<NoteId, IpcError> {
    s.parse()
        .map(NoteId)
        .map_err(|_| IpcError::Invalid(beholder_core::ValidationError::Character))
}

fn note_op(shell: &Shell, campaign_id: &str, op: NoteOp) -> Result<(), IpcError> {
    submit(
        shell,
        CommandKind::Note {
            campaign_id: self::campaign_id(campaign_id)?,
            op,
        },
    )
}

#[tauri::command(rename_all = "snake_case")]
pub fn create_note(
    shell: State<'_, Shell>,
    campaign_id: String,
    title: String,
    audience: Audience,
) -> Result<String, IpcError> {
    let id = NoteId(uuid::Uuid::now_v7());
    let op = NoteOp::Create {
        id,
        title: NoteTitle::new(&title).map_err(IpcError::Invalid)?,
        body: NoteBody::default(),
        audience,
    };
    note_op(&shell, &campaign_id, op)?;
    Ok(id.to_string())
}

#[tauri::command(rename_all = "snake_case")]
pub fn edit_note(
    shell: State<'_, Shell>,
    campaign_id: String,
    note_id: String,
    base_version: u32,
    title: String,
    body: String,
) -> Result<(), IpcError> {
    let op = NoteOp::Edit {
        id: self::note_id(&note_id)?,
        base_version,
        title: NoteTitle::new(&title).map_err(IpcError::Invalid)?,
        body: NoteBody::new(body).map_err(IpcError::Invalid)?,
    };
    note_op(&shell, &campaign_id, op)
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_note_audience(
    shell: State<'_, Shell>,
    campaign_id: String,
    note_id: String,
    audience: Audience,
) -> Result<(), IpcError> {
    let op = NoteOp::SetAudience {
        id: self::note_id(&note_id)?,
        audience,
    };
    note_op(&shell, &campaign_id, op)
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_note(
    shell: State<'_, Shell>,
    campaign_id: String,
    note_id: String,
) -> Result<(), IpcError> {
    let op = NoteOp::Delete {
        id: self::note_id(&note_id)?,
    };
    note_op(&shell, &campaign_id, op)
}

const SEARCH_LIMIT: usize = 300;

#[tauri::command(rename_all = "snake_case")]
pub fn compendium_search(
    shell: State<'_, Shell>,
    query: SearchQuery,
) -> Result<Vec<Hit>, IpcError> {
    shell
        .session()?
        .client
        .search(&query, SEARCH_LIMIT)
        .map_err(|_| IpcError::Storage)
}

#[tauri::command(rename_all = "snake_case")]
pub fn compendium_resolve(
    shell: State<'_, Shell>,
    items: Vec<ResolveItem>,
    sources: Vec<String>,
    hidden: Vec<String>,
) -> Result<Vec<Option<Hit>>, IpcError> {
    let client = shell.session()?.client;
    items
        .into_iter()
        .take(1000)
        .map(|item| {
            let kinds: Vec<Option<Kind>> = if item.kinds.is_empty() {
                vec![None]
            } else {
                item.kinds.into_iter().map(Some).collect()
            };
            for kind in kinds {
                let query = SearchQuery {
                    text: item.name.clone(),
                    kind,
                    sources: sources.clone(),
                    hidden: hidden.clone(),
                    ..SearchQuery::default()
                };
                let hits = client.search(&query, 10).map_err(|_| IpcError::Storage)?;
                if let Some(h) = hits
                    .into_iter()
                    .find(|h| h.name.trim().eq_ignore_ascii_case(item.name.trim()))
                {
                    return Ok(Some(h));
                }
            }
            Ok(None)
        })
        .collect()
}

#[tauri::command(rename_all = "snake_case")]
pub fn compendium_entry(
    shell: State<'_, Shell>,
    source: String,
    key: String,
    sources: Vec<String>,
) -> Result<Option<EntryView>, IpcError> {
    let client = shell.session()?.client;
    let found = client
        .compendium_entry(&source, &key)
        .map_err(|_| IpcError::Storage)?;
    let Some((entry, meta)) = found else {
        return Ok(None);
    };
    let also = client
        .alternatives(&source, &key, &sources)
        .map_err(|_| IpcError::Storage)?;
    Ok(Some(EntryView {
        entry,
        source: meta,
        also,
    }))
}

#[tauri::command(rename_all = "snake_case")]
pub fn compendium_credits(shell: State<'_, Shell>) -> Result<Credits, IpcError> {
    let client = shell.session()?.client;
    Ok(Credits {
        sources: client.compendium_sources().map_err(|_| IpcError::Storage)?,
        licenses: client
            .compendium_licenses()
            .map_err(|_| IpcError::Storage)?,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn compendium_available(shell: State<'_, Shell>) -> Result<Vec<SourceInfo>, IpcError> {
    let session = shell.session()?;
    let token = lock(&session.token).clone();
    let http = beholder_client_core::http::Http::new(&session.server_url);
    tauri::async_runtime::spawn_blocking(move || http.compendium_index(&token))
        .await
        .map_err(|_| IpcError::Server)?
        .map_err(http_error)
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_sources(
    shell: State<'_, Shell>,
    campaign_id: String,
    sources: Vec<String>,
) -> Result<(), IpcError> {
    campaign_op(&shell, &campaign_id, CampaignOp::SetSources { sources })
}

#[tauri::command(rename_all = "snake_case")]
pub fn compendium_facets(shell: State<'_, Shell>) -> Result<FacetOptions, IpcError> {
    shell
        .session()?
        .client
        .facet_options()
        .map_err(|_| IpcError::Storage)
}

#[tauri::command(rename_all = "snake_case")]
pub fn library_save(
    shell: State<'_, Shell>,
    mut draft: LibraryDraft,
    base_version: u32,
) -> Result<String, IpcError> {
    draft.check_facets().map_err(IpcError::Invalid)?;
    if base_version == 0 && draft.id.0.is_nil() {
        draft.id = LibraryId(uuid::Uuid::now_v7());
    }
    let id = draft.id.to_string();
    shell
        .session()?
        .client
        .library_save(draft, base_version)
        .map_err(|_| IpcError::Storage)?;
    Ok(id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn library_delete(shell: State<'_, Shell>, id: LibraryId) -> Result<(), IpcError> {
    shell
        .session()?
        .client
        .library_delete(id)
        .map_err(|_| IpcError::Storage)
}

#[tauri::command(rename_all = "snake_case")]
pub fn library_hide(
    shell: State<'_, Shell>,
    campaign_id: String,
    id: LibraryId,
    hidden: bool,
) -> Result<(), IpcError> {
    campaign_op(&shell, &campaign_id, CampaignOp::HideEntry { id, hidden })
}

fn client_error(e: ClientError) -> IpcError {
    match e {
        ClientError::Cache(_) => IpcError::Storage,
        ClientError::File(_) => IpcError::BadFile,
        ClientError::Http(_) => IpcError::Network,
        ClientError::Mapping(_) => IpcError::BadFile,
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn library_export(
    app: AppHandle,
    shell: State<'_, Shell>,
    yaml: bool,
) -> Result<String, IpcError> {
    let format = if yaml {
        FileFormat::Yaml
    } else {
        FileFormat::Json
    };
    let text = shell
        .session()?
        .client
        .library_export(format)
        .map_err(client_error)?;
    let dir = app.path().download_dir().map_err(|_| IpcError::Storage)?;
    let ext = if yaml { "yaml" } else { "json" };
    let mut path = dir.join(format!("beholder-library.{ext}"));
    let mut n = 1;
    while path.exists() {
        n += 1;
        path = dir.join(format!("beholder-library-{n}.{ext}"));
    }
    std::fs::write(&path, text).map_err(|_| IpcError::Storage)?;
    Ok(path.display().to_string())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn library_import(
    shell: State<'_, Shell>,
    request: Request<'_>,
) -> Result<ImportSummary, IpcError> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(IpcError::BadFile);
    };
    let format = match request
        .headers()
        .get("file-format")
        .and_then(|v| v.to_str().ok())
    {
        Some("yaml") => FileFormat::Yaml,
        _ => FileFormat::Json,
    };
    let replace = request
        .headers()
        .get("replace")
        .and_then(|v| v.to_str().ok())
        == Some("1");
    let client = shell.session()?.client;
    let bytes = bytes.clone();
    tauri::async_runtime::spawn_blocking(move || client.library_import(&bytes, format, replace))
        .await
        .map_err(|_| IpcError::Server)?
        .map_err(client_error)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn library_paste(
    shell: State<'_, Shell>,
    text: String,
    book: Option<String>,
    rules: RulesVersion,
    kind: Kind,
) -> Result<Vec<PasteItem>, IpcError> {
    let client = shell.session()?.client;
    let previews = tauri::async_runtime::spawn_blocking(move || {
        client.library_paste(&text, book.as_deref(), rules, kind)
    })
    .await
    .map_err(|_| IpcError::Server)?
    .map_err(client_error)?;
    Ok(previews
        .into_iter()
        .map(|p| PasteItem {
            entry: p.entry,
            duplicate: p.duplicate,
        })
        .collect())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn library_accept(
    shell: State<'_, Shell>,
    entries: Vec<Pasted>,
    book: Option<String>,
    rules: RulesVersion,
    replace: bool,
) -> Result<ImportSummary, IpcError> {
    let client = shell.session()?.client;
    tauri::async_runtime::spawn_blocking(move || {
        client.library_accept(entries, book.as_deref(), rules, replace)
    })
    .await
    .map_err(|_| IpcError::Server)?
    .map_err(client_error)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn library_imports(shell: State<'_, Shell>) -> Result<Vec<ImportInfo>, IpcError> {
    let client = shell.session()?.client;
    tauri::async_runtime::spawn_blocking(move || client.library_imports())
        .await
        .map_err(|_| IpcError::Server)?
        .map_err(client_error)
}

#[tauri::command(rename_all = "snake_case")]
pub fn library_undo_import(shell: State<'_, Shell>, import: ImportId) -> Result<(), IpcError> {
    shell
        .session()?
        .client
        .library_undo_import(import)
        .map_err(client_error)
}

fn pick_json(app: &AppHandle) -> Option<std::path::PathBuf> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog()
        .file()
        .add_filter("JSON", &["json"])
        .blocking_pick_file()
        .and_then(|f| f.into_path().ok())
}

fn bulk_path(shell: &Shell) -> Result<std::path::PathBuf, IpcError> {
    lock(&shell.bulk)
        .as_ref()
        .map(|b| b.path.clone())
        .ok_or(IpcError::BadFile)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn bulk_open(
    app: AppHandle,
    shell: State<'_, Shell>,
) -> Result<Option<BulkOpened>, IpcError> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let path = pick_json(&app)?;
        Some(beholder_client_core::bulk_probe(&path).map(|found| (path, found)))
    })
    .await
    .map_err(|_| IpcError::Server)?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let (path, (probe, profile)) = picked.map_err(client_error)?;
    let opened = BulkOpened {
        file_name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        bytes: std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
        keys: beholder_client_core::mapping::leaf_keys(&probe.sample),
        sample: probe
            .sample
            .iter()
            .take(3)
            .filter_map(|v| serde_json::to_string_pretty(v).ok())
            .map(|s| s.chars().take(4000).collect())
            .collect(),
        profile,
    };
    *lock(&shell.bulk) = Some(BulkFile {
        path,
        sample: probe.sample,
    });
    Ok(Some(opened))
}

#[tauri::command(rename_all = "snake_case")]
pub fn bulk_values(shell: State<'_, Shell>, key: String) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(b) = lock(&shell.bulk).as_ref() {
        for v in &b.sample {
            let value = beholder_client_core::mapping::get(v, &key).and_then(|x| match x {
                serde_json::Value::String(s) => Some(s.trim().to_lowercase()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                _ => None,
            });
            if let Some(value) = value.filter(|v| !v.is_empty() && !out.contains(v)) {
                out.push(value);
            }
        }
    }
    out
}

#[tauri::command(rename_all = "snake_case")]
pub fn bulk_preview(shell: State<'_, Shell>, profile: Profile) -> Result<Vec<PasteItem>, IpcError> {
    let sample = lock(&shell.bulk)
        .as_ref()
        .map(|b| b.sample.clone())
        .ok_or(IpcError::BadFile)?;
    let previews = shell
        .session()?
        .client
        .bulk_preview(&sample, &profile)
        .map_err(client_error)?;
    Ok(previews
        .into_iter()
        .map(|p| PasteItem {
            entry: p.entry,
            duplicate: p.duplicate,
        })
        .collect())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn bulk_scan(
    shell: State<'_, Shell>,
    profile: Profile,
) -> Result<BulkScanView, IpcError> {
    let path = bulk_path(&shell)?;
    let client = shell.session()?.client;
    let s = tauri::async_runtime::spawn_blocking(move || client.bulk_scan(&path, &profile))
        .await
        .map_err(|_| IpcError::Server)?
        .map_err(client_error)?;
    Ok(BulkScanView {
        total: s.total,
        kinds: s.kinds,
        books: s.books,
        duplicates: s.duplicates,
        partial: s.partial,
        invalid: s.invalid,
        invalid_count: s.invalid_count,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn bulk_import(
    shell: State<'_, Shell>,
    profile: Profile,
    replace: bool,
) -> Result<ImportSummary, IpcError> {
    let path = bulk_path(&shell)?;
    let client = shell.session()?.client;
    tauri::async_runtime::spawn_blocking(move || client.bulk_import(&path, &profile, replace))
        .await
        .map_err(|_| IpcError::Server)?
        .map_err(client_error)
}

#[tauri::command(rename_all = "snake_case")]
pub fn mapping_profiles(shell: State<'_, Shell>) -> Vec<Profile> {
    load_profiles(&shell.config_dir)
}

#[tauri::command(rename_all = "snake_case")]
pub fn mapping_profile_save(
    shell: State<'_, Shell>,
    profile: Profile,
) -> Result<Vec<Profile>, IpcError> {
    if profile.name.trim().is_empty() {
        return Err(IpcError::Invalid(beholder_core::ValidationError::Length));
    }
    let mut all = load_profiles(&shell.config_dir);
    all.retain(|p| p.name != profile.name);
    all.push(profile);
    all.sort_by_key(|p| p.name.to_lowercase());
    save_profiles(&shell.config_dir, &all).map_err(|_| IpcError::Storage)?;
    Ok(all)
}

#[tauri::command(rename_all = "snake_case")]
pub fn mapping_profile_delete(
    shell: State<'_, Shell>,
    name: String,
) -> Result<Vec<Profile>, IpcError> {
    let mut all = load_profiles(&shell.config_dir);
    all.retain(|p| p.name != name);
    save_profiles(&shell.config_dir, &all).map_err(|_| IpcError::Storage)?;
    Ok(all)
}

#[tauri::command(rename_all = "snake_case")]
pub fn mapping_profile_export(
    app: AppHandle,
    shell: State<'_, Shell>,
    name: String,
) -> Result<String, IpcError> {
    let profile = load_profiles(&shell.config_dir)
        .into_iter()
        .find(|p| p.name == name)
        .ok_or(IpcError::BadFile)?;
    let dir = app.path().download_dir().map_err(|_| IpcError::Storage)?;
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let mut path = dir.join(format!("beholder-mapping-{slug}.json"));
    let mut n = 1;
    while path.exists() {
        n += 1;
        path = dir.join(format!("beholder-mapping-{slug}-{n}.json"));
    }
    let text = serde_json::to_vec_pretty(&profile).map_err(|_| IpcError::Storage)?;
    std::fs::write(&path, text).map_err(|_| IpcError::Storage)?;
    Ok(path.display().to_string())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn mapping_profile_import(
    app: AppHandle,
    shell: State<'_, Shell>,
) -> Result<Vec<Profile>, IpcError> {
    let picked = tauri::async_runtime::spawn_blocking(move || pick_json(&app))
        .await
        .map_err(|_| IpcError::Server)?;
    let Some(path) = picked else {
        return Ok(load_profiles(&shell.config_dir));
    };
    let bytes = std::fs::read(&path).map_err(|_| IpcError::BadFile)?;
    let profile: Profile = serde_json::from_slice(&bytes).map_err(|_| IpcError::BadFile)?;
    mapping_profile_save(shell, profile)
}

#[tauri::command(rename_all = "snake_case")]
pub fn character_command(
    shell: State<'_, Shell>,
    campaign_id: String,
    op: CharacterOp,
) -> Result<(), IpcError> {
    let campaign_id = self::campaign_id(&campaign_id)?;
    shell
        .session()?
        .client
        .submit(CommandKind::Character { campaign_id, op })
        .map(|_| ())
        .map_err(|_| IpcError::Storage)
}

#[tauri::command(rename_all = "snake_case")]
pub fn character_create(
    shell: State<'_, Shell>,
    campaign_id: String,
    name: String,
    rules: RulesVersion,
) -> Result<String, IpcError> {
    let id = beholder_core::CharacterId(uuid::Uuid::now_v7());
    character_command(shell, campaign_id, CharacterOp::Create { id, name, rules })?;
    Ok(id.to_string())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CharacterFile {
    format: String,
    version: u32,
    character: beholder_core::character::Character,
}

const CHARACTER_FORMAT: &str = "beholder-character";

#[tauri::command(rename_all = "snake_case")]
pub fn character_export(
    app: AppHandle,
    shell: State<'_, Shell>,
    campaign_id: String,
    id: beholder_core::CharacterId,
) -> Result<String, IpcError> {
    let campaign = self::campaign_id(&campaign_id)?;
    let view = shell
        .session()?
        .client
        .view()
        .map_err(|_| IpcError::Storage)?;
    let character = view
        .campaigns
        .into_iter()
        .find(|c| c.campaign.id == campaign)
        .and_then(|c| c.campaign.character(id).cloned())
        .ok_or(IpcError::BadFile)?;
    let slug: String = character
        .name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let file = CharacterFile {
        format: CHARACTER_FORMAT.into(),
        version: 1,
        character,
    };
    let dir = app.path().download_dir().map_err(|_| IpcError::Storage)?;
    let mut path = dir.join(format!("beholder-character-{slug}.json"));
    let mut n = 1;
    while path.exists() {
        n += 1;
        path = dir.join(format!("beholder-character-{slug}-{n}.json"));
    }
    let text = serde_json::to_vec_pretty(&file).map_err(|_| IpcError::Storage)?;
    std::fs::write(&path, text).map_err(|_| IpcError::Storage)?;
    Ok(path.display().to_string())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn character_import(
    app: AppHandle,
    shell: State<'_, Shell>,
    campaign_id: String,
) -> Result<Option<String>, IpcError> {
    let picked = tauri::async_runtime::spawn_blocking(move || pick_json(&app))
        .await
        .map_err(|_| IpcError::Server)?;
    let Some(path) = picked else {
        return Ok(None);
    };
    let bytes = std::fs::read(&path).map_err(|_| IpcError::BadFile)?;
    let file: CharacterFile = serde_json::from_slice(&bytes).map_err(|_| IpcError::BadFile)?;
    if file.format != CHARACTER_FORMAT || file.version != 1 {
        return Err(IpcError::BadFile);
    }
    let source = file.character;
    source.validate().map_err(IpcError::Invalid)?;
    let campaign = self::campaign_id(&campaign_id)?;
    let client = shell.session()?.client;
    let id = beholder_core::CharacterId(uuid::Uuid::now_v7());
    let submit = |op| {
        client
            .submit(CommandKind::Character {
                campaign_id: campaign,
                op,
            })
            .map(|_| ())
            .map_err(|_| IpcError::Storage)
    };
    submit(CharacterOp::Create {
        id,
        name: source.name.clone(),
        rules: source.rules,
    })?;
    for change in source.to_changes() {
        submit(CharacterOp::Change { id, change })?;
    }
    Ok(Some(id.to_string()))
}
