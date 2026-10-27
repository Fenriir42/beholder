use std::path::PathBuf;

use beholder_client_core::http::{Http, ImageVariant};
use beholder_core::ImageHash;
use tauri::http::{Response, StatusCode, header};
use tauri::{AppHandle, Manager, UriSchemeResponder};

use crate::session::{Shell, lock};

pub fn handle(app: AppHandle, path: String, responder: UriSchemeResponder) {
    tauri::async_runtime::spawn(async move {
        let response = match serve(&app, &path).await {
            Some(bytes) => Response::builder()
                .header(header::CONTENT_TYPE, "image/webp")
                .header(header::CACHE_CONTROL, "max-age=31536000, immutable")
                .body(bytes),
            None => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Vec::new()),
        };
        if let Ok(r) = response {
            responder.respond(r);
        }
    });
}

async fn serve(app: &AppHandle, path: &str) -> Option<Vec<u8>> {
    let (variant, hex) = path.trim_start_matches('/').split_once('/')?;
    let variant = match variant {
        "thumb" => ImageVariant::Thumb,
        "display" => ImageVariant::Display,
        _ => return None,
    };
    let hash = ImageHash::from_hex(hex)?;
    let shell = app.state::<Shell>();
    let file: PathBuf = shell.image_dir.join(format!(
        "{}-{}.webp",
        hash.to_hex(),
        if variant == ImageVariant::Thumb {
            "thumb"
        } else {
            "display"
        }
    ));
    if let Ok(bytes) = tokio_fs_read(&file).await {
        return Some(bytes);
    }
    let url = lock(&shell.config).server_url.clone()?;
    let dir = shell.image_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = Http::new(&url).image(&hash, variant).ok()?;
        let _ = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&file, &bytes));
        Some(bytes)
    })
    .await
    .ok()?
}

async fn tokio_fs_read(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    let path = path.to_owned();
    tauri::async_runtime::spawn_blocking(move || std::fs::read(path))
        .await
        .map_err(std::io::Error::other)?
}
