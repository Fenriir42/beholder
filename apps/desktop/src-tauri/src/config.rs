//! Small persisted settings: server URL, UI language, last user. Secrets are
//! never stored here; the session token lives in the OS keyring.

use std::path::{Path, PathBuf};

use beholder_core::UserId;
use serde::{Deserialize, Serialize};

/// What the shell remembers between launches.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    /// The server, normalized (`https://host[:port]`, no trailing slash).
    pub server_url: Option<String>,
    /// UI language code.
    pub locale: Option<String>,
    /// The last signed-in user, to open their cache before the network answers.
    pub last_user: Option<UserId>,
}

impl Config {
    fn path(dir: &Path) -> PathBuf {
        dir.join("config.json")
    }

    /// Loads the config; a missing or unreadable file means defaults.
    pub fn load(dir: &Path) -> Self {
        std::fs::read(Self::path(dir))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Saves the config (temp file + rename, so a crash can't corrupt it).
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join("config.json.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?,
        )?;
        std::fs::rename(tmp, Self::path(dir))
    }
}

/// Normalizes a server address: `http(s)://host[:port]`, no path. Bare hosts
/// get `https://`.
pub fn normalize_server_url(input: &str) -> Option<String> {
    let s = input.trim().trim_end_matches('/');
    let (scheme, rest) = match s.split_once("://") {
        Some((scheme, rest)) => (scheme.to_ascii_lowercase(), rest),
        None => ("https".to_owned(), s),
    };
    if scheme != "https" && scheme != "http" {
        return None;
    }
    let valid = !rest.is_empty()
        && rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'));
    valid.then(|| format!("{scheme}://{}", rest.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_urls_are_normalized() {
        assert_eq!(
            normalize_server_url(" dnd.example.org/ ").as_deref(),
            Some("https://dnd.example.org")
        );
        assert_eq!(
            normalize_server_url("HTTP://127.0.0.1:8080").as_deref(),
            Some("http://127.0.0.1:8080")
        );
        assert_eq!(normalize_server_url("ftp://x"), None);
        assert_eq!(normalize_server_url("https://host/path"), None);
        assert_eq!(normalize_server_url(""), None);
    }
}
