use std::sync::atomic::Ordering;
use std::time::Duration;

use beholder_core::compendium::{BUNDLE_FORMAT, Bundle};
use beholder_protocol::SessionToken;
use beholder_protocol::http::MAX_BUNDLE_BYTES;
use sha2::{Digest, Sha256};

use crate::cache::CacheError;
use crate::http::{BundlePart, Http, HttpError};
use crate::{ClientEvent, CompendiumState, Shared};

const ATTEMPTS: u32 = 6;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("http: {0}")]
    Http(#[from] HttpError),
    #[error("cache: {0}")]
    Cache(#[from] CacheError),
    #[error("bundle {0} doesn't match its hash")]
    Hash(String),
    #[error("bundle {0} is corrupt or in an unknown format")]
    Corrupt(String),
}

pub(crate) fn run(shared: &Shared, server_url: &str, token: &SessionToken) {
    if shared.compendium_busy.swap(true, Ordering::AcqRel) {
        return;
    }
    shared.emit(ClientEvent::Compendium(CompendiumState::Syncing));
    let state = match sync(shared, &Http::new(server_url), token) {
        Ok(()) => {
            shared.compendium_done.store(true, Ordering::Release);
            CompendiumState::Ready
        }
        Err(_) => CompendiumState::Failed,
    };
    shared.compendium_busy.store(false, Ordering::Release);
    shared.emit(ClientEvent::Compendium(state));
}

fn sync(shared: &Shared, http: &Http, token: &SessionToken) -> Result<(), SyncError> {
    let index = http.compendium_index(token)?;
    for src in &index {
        if shared
            .with_cache(|c| c.compendium_sha256(&src.id))?
            .as_deref()
            == Some(&src.sha256)
        {
            continue;
        }
        let bytes = download(http, token, &src.sha256)?;
        let bundle = verify(&src.sha256, &bytes)?;
        shared.with_cache(|c| c.put_bundle(&src.sha256, &bundle))?;
    }
    let ids: Vec<String> = index.into_iter().map(|s| s.id).collect();
    shared.with_cache(|c| c.retain_sources(&ids))?;
    Ok(())
}

fn download(http: &Http, token: &SessionToken, sha256: &str) -> Result<Vec<u8>, SyncError> {
    let mut have = Vec::new();
    let mut last = None;
    for attempt in 0..ATTEMPTS {
        if attempt > 0 {
            std::thread::sleep(Duration::from_secs(1 << attempt.min(4)));
        }
        match http.bundle_from(token, sha256, have.len()) {
            Ok(BundlePart::Whole(b)) => return Ok(b),
            Ok(BundlePart::Rest(b)) => {
                have.extend_from_slice(&b);
                return Ok(have);
            }
            Err(HttpError::Status(s)) if s == 401 || s == 404 => {
                return Err(HttpError::Status(s).into());
            }
            Err(e) => last = Some(e),
        }
    }
    Err(last.map_or_else(|| SyncError::Hash(sha256.to_owned()), SyncError::Http))
}

fn verify(sha256: &str, bytes: &[u8]) -> Result<Bundle, SyncError> {
    let digest: String = Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if digest != sha256 {
        return Err(SyncError::Hash(sha256.to_owned()));
    }
    let raw = zstd::bulk::decompress(bytes, MAX_BUNDLE_BYTES)
        .map_err(|_| SyncError::Corrupt(sha256.to_owned()))?;
    match postcard::from_bytes::<Bundle>(&raw) {
        Ok(b) if b.format == BUNDLE_FORMAT => Ok(b),
        _ => Err(SyncError::Corrupt(sha256.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundles_are_verified() {
        let bytes = include_bytes!("../../../data/srd-5.2/srd.bundle");
        let sha: String = Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert!(verify(&sha, bytes).is_ok());
        assert!(matches!(
            verify(&"0".repeat(64), bytes),
            Err(SyncError::Hash(_))
        ));
        let junk = b"not a bundle";
        let junk_sha: String = Sha256::digest(junk)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert!(matches!(
            verify(&junk_sha, junk),
            Err(SyncError::Corrupt(_))
        ));
    }
}
