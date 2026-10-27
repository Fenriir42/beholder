use axum::body::Bytes;
use beholder_core::compendium::{BUNDLE_FORMAT, SourceMeta};
use beholder_protocol::http::SourceInfo;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const SRD: &[u8] = include_bytes!("../../../data/srd-5.2/srd.bundle");

mod bundled {
    include!(concat!(env!("OUT_DIR"), "/bundles.rs"));
}

const HEAD_BYTES: u64 = 512 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("bundle doesn't decompress: {0}")]
    Decompress(#[from] std::io::Error),
    #[error("bundle header doesn't decode: {0}")]
    Decode(#[from] postcard::Error),
    #[error("bundle format {0}, expected {BUNDLE_FORMAT}")]
    Format(u16),
}

// Must mirror the leading fields of compendium::Bundle, in order: postcard decodes by position.
#[derive(Deserialize)]
struct Head {
    format: u16,
    _seq: u64,
    source: SourceMeta,
}

pub struct CachedBundle {
    pub info: SourceInfo,
    pub seq: u64,
    pub path: std::path::PathBuf,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub struct Library {
    sources: Vec<(SourceInfo, Bytes)>,
}

impl Library {
    pub fn builtin() -> Result<Self, BundleError> {
        let mut sources = vec![describe(Bytes::from_static(SRD))?];
        for b in bundled::OPEN5E {
            sources.push(describe(Bytes::from_static(b))?);
        }
        Ok(Self { sources })
    }

    pub fn index(&self) -> Vec<SourceInfo> {
        self.sources.iter().map(|(info, _)| info.clone()).collect()
    }

    pub fn has(&self, id: &str) -> bool {
        self.sources.iter().any(|(info, _)| info.id == id)
    }

    pub fn bundle(&self, sha256: &str) -> Option<Bytes> {
        self.sources
            .iter()
            .find(|(info, _)| info.sha256 == sha256)
            .map(|(_, b)| b.clone())
    }
}

fn describe(bytes: Bytes) -> Result<(SourceInfo, Bytes), BundleError> {
    let mut raw = Vec::new();
    std::io::Read::read_to_end(
        &mut std::io::Read::take(zstd::stream::read::Decoder::new(&bytes[..])?, HEAD_BYTES),
        &mut raw,
    )?;
    let (head, _) = postcard::take_from_bytes::<Head>(&raw)?;
    if head.format != BUNDLE_FORMAT {
        return Err(BundleError::Format(head.format));
    }
    let info = SourceInfo {
        id: head.source.id,
        name: head.source.name,
        publisher: head.source.publisher,
        system: head.source.system,
        rules: head.source.rules,
        sha256: sha256_hex(&bytes),
        bytes: bytes.len() as u64,
    };
    Ok((info, bytes))
}

pub fn parse_range(header: &str, len: usize) -> Option<usize> {
    let start: usize = header
        .strip_prefix("bytes=")?
        .strip_suffix('-')?
        .parse()
        .ok()?;
    (start < len).then_some(start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_srd_is_valid() {
        let lib = Library::builtin().unwrap();
        let index = lib.index();
        assert!(index.len() > 1, "SRD plus the Open5e sources");
        assert_eq!(index[0].id, "srd-5.2");
        assert!(lib.has("tob3") && lib.has("vom"));
        assert!(!lib.has("tob"), "superseded edition is skipped");
        assert_eq!(index[0].sha256.len(), 64);
        assert_eq!(
            lib.bundle(&index[0].sha256).map(|b| b.len() as u64),
            Some(index[0].bytes)
        );
        assert!(lib.bundle(&"0".repeat(64)).is_none());
    }

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=10-", 100), Some(10));
        assert_eq!(parse_range("bytes=0-", 100), Some(0));
        assert_eq!(parse_range("bytes=100-", 100), None);
        assert_eq!(
            parse_range("bytes=1-5", 100),
            None,
            "only open-ended ranges"
        );
        assert_eq!(parse_range("items=1-", 100), None);
    }
}
