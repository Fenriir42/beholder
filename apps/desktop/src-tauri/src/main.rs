//! Beholder desktop shell: hosts the Leptos UI and bridges it to client-core
//! through Tauri commands and events. The UI never touches the network.

// Release builds on Windows shouldn't open a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod images;
mod secrets;
mod session;

use std::sync::Mutex;

use tauri::Manager;

use crate::config::Config;
use crate::secrets::Secrets;
use crate::session::Shell;

fn main() {
    let result = tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol("img", |ctx, request, responder| {
            images::handle(
                ctx.app_handle().clone(),
                request.uri().path().to_owned(),
                responder,
            );
        })
        .setup(|app| {
            let paths = app.path();
            let shell = Shell {
                config_dir: paths.app_config_dir()?,
                data_dir: paths.app_data_dir()?,
                image_dir: paths.app_cache_dir()?.join("images"),
                config: Mutex::new(Config::default()),
                secrets: Secrets::init(),
                session: Mutex::new(None),
            };
            let config = Config::load(&shell.config_dir);
            *session::lock(&shell.config) = config.clone();
            app.manage(shell);
            // Resume the last session from the cache: the library renders before
            // the network answers (SPEC §2, cold start < 1 s).
            if let (Some(url), Some(user)) = (config.server_url, config.last_user) {
                let shell = app.state::<Shell>();
                if let Some(token) = shell.secrets.load(&url) {
                    let handle = app.handle().clone();
                    tauri::async_runtime::block_on(async {
                        let _ = shell.start(&handle, url, user, token);
                    });
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::boot,
            commands::connection_state,
            commands::set_server_url,
            commands::set_locale,
            commands::login,
            commands::register,
            commands::logout,
            commands::view,
            commands::retry_now,
            commands::create_campaign,
            commands::join_campaign,
            commands::rename_campaign,
            commands::update_settings,
            commands::regenerate_invite,
            commands::upload_cover,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("beholder: {e}");
        std::process::exit(1);
    }
}
