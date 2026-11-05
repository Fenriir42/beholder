use std::path::{Component, Path, PathBuf};

use axum::extract::State;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Redirect, Response};

use crate::AppState;

const PREFIX: &str = "/docs";

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "wasm" => "application/wasm",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml",
        _ => "application/octet-stream",
    }
}

// Only plain path segments under the docs root: no `..`, no absolute parts.
pub(crate) fn resolve(root: &Path, request: &str) -> Option<PathBuf> {
    let rel = request.strip_prefix(PREFIX)?;
    if !(rel.is_empty() || rel.starts_with('/')) {
        return None;
    }
    let mut out = root.to_path_buf();
    for part in Path::new(rel.trim_start_matches('/')).components() {
        match part {
            Component::Normal(p) => out.push(p),
            _ => return None,
        }
    }
    if rel.is_empty() || rel.ends_with('/') {
        out.push("index.html");
    }
    Some(out)
}

fn file(status: StatusCode, path: &Path, bytes: Vec<u8>) -> Response {
    let immutable = path.components().any(|c| c.as_os_str() == "_astro");
    let cache = if immutable {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    (
        status,
        [
            (header::CONTENT_TYPE, content_type(path)),
            (header::CACHE_CONTROL, cache),
        ],
        bytes,
    )
        .into_response()
}

pub async fn page(State(state): State<AppState>, uri: Uri) -> Response {
    let Some(root) = state.docs_dir.as_deref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let request = uri.path();
    if request == PREFIX {
        return Redirect::permanent("/docs/").into_response();
    }
    let Some(path) = resolve(root, request) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if let Ok(bytes) = tokio::fs::read(&path).await {
        return file(StatusCode::OK, &path, bytes);
    }
    if tokio::fs::metadata(&path).await.is_ok_and(|m| m.is_dir()) {
        return Redirect::permanent(&format!("{request}/")).into_response();
    }
    let missing = root.join("404.html");
    match tokio::fs::read(&missing).await {
        Ok(bytes) => file(StatusCode::NOT_FOUND, &missing, bytes),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn home(State(state): State<AppState>) -> Response {
    if state.docs_dir.is_some() {
        Redirect::temporary("/docs/").into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_stay_inside_the_docs() {
        let root = Path::new("/srv/docs");
        assert_eq!(resolve(root, "/docs/"), Some(root.join("index.html")));
        assert_eq!(resolve(root, "/docs"), Some(root.join("index.html")));
        assert_eq!(
            resolve(root, "/docs/fr/players/combat/"),
            Some(root.join("fr/players/combat/index.html"))
        );
        assert_eq!(
            resolve(root, "/docs/_astro/app.js"),
            Some(root.join("_astro/app.js"))
        );
        assert_eq!(resolve(root, "/docs/../beholder.db"), None);
        assert_eq!(resolve(root, "/docs/a/../../x"), None);
        assert_eq!(resolve(root, "/docsx/index.html"), None);
        assert_eq!(resolve(root, "/api/login"), None);
    }

    #[tokio::test]
    async fn pages_redirects_and_missing_files() {
        let data = tempfile::tempdir().unwrap();
        let docs = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(docs.path().join("players")).unwrap();
        std::fs::write(docs.path().join("index.html"), "home").unwrap();
        std::fs::write(docs.path().join("players/index.html"), "players").unwrap();
        std::fs::write(docs.path().join("404.html"), "lost").unwrap();
        let state = crate::Server::open(data.path().to_path_buf(), Some(docs.path().to_path_buf()))
            .unwrap();
        let get = |p: &'static str| page(State(std::sync::Arc::clone(&state)), Uri::from_static(p));
        let ok = get("/docs/players/").await;
        assert_eq!(ok.status(), StatusCode::OK);
        assert_eq!(
            ok.headers()[header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
        assert_eq!(
            get("/docs/players").await.status(),
            StatusCode::PERMANENT_REDIRECT
        );
        assert_eq!(get("/docs").await.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(get("/docs/nope/").await.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            get("/docs/../beholder.db").await.status(),
            StatusCode::NOT_FOUND
        );
        let off = crate::Server::open(tempfile::tempdir().unwrap().keep(), None).unwrap();
        assert_eq!(home(State(off)).await.status(), StatusCode::NOT_FOUND);
    }
}
