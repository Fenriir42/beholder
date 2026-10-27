use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use beholder_core::compendium::{BUNDLE_FORMAT, Bundle, LIBRARY};
use beholder_protocol::SessionToken;
use beholder_protocol::http::MAX_BUNDLE_BYTES;
use sha2::{Digest, Sha256};

use crate::cache::{Cache, CacheError};
use crate::http::{BundlePart, Http, HttpError};
use crate::{ClientEvent, CompendiumState, Shared, lock};

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

fn wanted(cache: &Cache) -> Result<Vec<String>, CacheError> {
    let mut all: Vec<String> = cache
        .campaigns()?
        .into_iter()
        .flat_map(|(_, c)| c.sources)
        .collect();
    all.sort();
    all.dedup();
    Ok(all)
}

pub(crate) fn maybe_start(shared: &Arc<Shared>, token: &SessionToken) {
    let Ok(want) = shared.with_cache(wanted) else {
        return;
    };
    let dirty = shared.library_dirty.load(Ordering::Acquire) && want.iter().any(|s| s == LIBRARY);
    if (!dirty && lock(&shared.compendium_synced).as_ref() == Some(&want))
        || shared.compendium_busy.swap(true, Ordering::AcqRel)
    {
        return;
    }
    let (s, t) = (Arc::clone(shared), token.clone());
    tokio::task::spawn_blocking(move || run(&s, &t));
}

fn run(shared: &Shared, token: &SessionToken) {
    shared.emit(ClientEvent::Compendium(CompendiumState::Syncing));
    let http = Http::new(&shared.server_url);
    let state = loop {
        let Ok(want) = shared.with_cache(wanted) else {
            break CompendiumState::Failed;
        };
        if sync(shared, &http, token, &want).is_err() {
            break CompendiumState::Failed;
        }
        let settled = shared.with_cache(wanted).ok().as_ref() == Some(&want)
            && !shared.library_dirty.load(Ordering::Acquire);
        *lock(&shared.compendium_synced) = Some(want);
        if settled {
            break CompendiumState::Ready;
        }
    };
    shared.compendium_busy.store(false, Ordering::Release);
    shared.emit(ClientEvent::Compendium(state));
}

fn sync(
    shared: &Shared,
    http: &Http,
    token: &SessionToken,
    want: &[String],
) -> Result<(), SyncError> {
    let index: Vec<_> = if want.is_empty() {
        Vec::new()
    } else {
        http.compendium_index(token)?
            .into_iter()
            .filter(|s| want.contains(&s.id))
            .collect()
    };
    shared.library_dirty.store(false, Ordering::Release);
    for src in &index {
        // The library bundle isn't rebuilt for every change, so its position decides, not its hash.
        if src.id == LIBRARY
            && let Some(seq) = shared.with_cache(|c| c.compendium_seq(LIBRARY))?
        {
            let delta = http.library_changes(token, seq)?;
            if let Some(changes) = delta.changes {
                shared.with_cache(|c| c.apply_library(&changes, delta.head))?;
                continue;
            }
        } else if shared
            .with_cache(|c| c.compendium_sha256(&src.id))?
            .as_deref()
            == Some(&src.sha256)
        {
            continue;
        }
        let bytes = download(http, token, &src.sha256)?;
        let bundle = verify(&src.sha256, &bytes)?;
        shared.with_cache(|c| c.put_bundle(&src.sha256, &bundle))?;
        if src.id == LIBRARY {
            let delta = http.library_changes(token, bundle.seq)?;
            if let Some(changes) = delta.changes {
                shared.with_cache(|c| c.apply_library(&changes, delta.head))?;
            }
        }
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
