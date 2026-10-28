use std::time::Duration;

use beholder_core::ImageHash;
use beholder_protocol::http::{
    AuthError, AuthResponse, ImageError, ImageUploaded, ImportInfo, LibraryChanges, LoginRequest,
    MAX_BUNDLE_BYTES, MAX_IMAGE_BYTES, RegisterRequest, SourceInfo,
};
use beholder_protocol::{PROTOCOL_HEADER, PROTOCOL_VERSION, SessionToken};
use serde::Serialize;
use serde::de::DeserializeOwned;
use ureq::Agent;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

const SHORT: Duration = Duration::from_secs(30);

const LONG: Duration = Duration::from_secs(15 * 60);

const SMALL_BODY: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageVariant {
    Thumb,
    Display,
}

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("auth: {0:?}")]
    Auth(AuthError),
    #[error("image: {0:?}")]
    Image(ImageError),
    #[error("HTTP {0}")]
    Status(u16),
    #[error("network: {0}")]
    Network(String),
    #[error("bad response")]
    Decode,
}

impl From<ureq::Error> for HttpError {
    fn from(e: ureq::Error) -> Self {
        Self::Network(e.to_string())
    }
}

pub struct Http {
    agent: Agent,
    base: String,
}

impl Http {
    pub fn new(server_url: &str) -> Self {
        let config = Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(SHORT))
            .tls_config(
                TlsConfig::builder()
                    .provider(TlsProvider::NativeTls)
                    .root_certs(RootCerts::PlatformVerifier)
                    .build(),
            )
            .build();
        Self {
            agent: config.into(),
            base: server_url.trim_end_matches('/').to_owned(),
        }
    }

    fn post(
        &self,
        path: &str,
        token: Option<&SessionToken>,
        body: &[u8],
        long: bool,
        limit: u64,
    ) -> Result<(u16, Vec<u8>), HttpError> {
        let mut req = self
            .agent
            .post(format!("{}{path}", self.base))
            .header(PROTOCOL_HEADER, PROTOCOL_VERSION.to_string())
            .header("content-type", "application/x-postcard");
        if let Some(t) = token {
            req = req.header("authorization", format!("Bearer {}", t.to_hex()));
        }
        if long {
            req = req.config().timeout_global(Some(LONG)).build();
        }
        let mut resp = req.send(body)?;
        let status = resp.status().as_u16();
        let bytes = resp.body_mut().with_config().limit(limit).read_to_vec()?;
        Ok((status, bytes))
    }

    fn auth<T: Serialize>(&self, path: &str, req: &T) -> Result<AuthResponse, HttpError> {
        let body = postcard::to_stdvec(req).map_err(|_| HttpError::Decode)?;
        let (status, bytes) = self.post(path, None, &body, false, SMALL_BODY)?;
        decode_or_error(status, &bytes, HttpError::Auth)
    }

    pub fn register(&self, req: &RegisterRequest) -> Result<AuthResponse, HttpError> {
        self.auth("/api/register", req)
    }

    pub fn login(&self, req: &LoginRequest) -> Result<AuthResponse, HttpError> {
        self.auth("/api/login", req)
    }

    pub fn logout(&self, token: &SessionToken) -> Result<(), HttpError> {
        let (status, _) = self.post("/api/logout", Some(token), &[], false, SMALL_BODY)?;
        if (200..300).contains(&status) {
            Ok(())
        } else {
            Err(HttpError::Status(status))
        }
    }

    pub fn upload_image(&self, token: &SessionToken, bytes: &[u8]) -> Result<ImageHash, HttpError> {
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(HttpError::Image(ImageError::TooLarge));
        }
        let (status, body) = self.post("/api/images", Some(token), bytes, true, SMALL_BODY)?;
        decode_or_error::<ImageUploaded, _>(status, &body, HttpError::Image).map(|u| u.hash)
    }

    pub fn image(&self, hash: &ImageHash, variant: ImageVariant) -> Result<Vec<u8>, HttpError> {
        let file = match variant {
            ImageVariant::Thumb => "thumb.webp",
            ImageVariant::Display => "display.webp",
        };
        let url = format!("{}/img/{}/{file}", self.base, hash.to_hex());
        let mut resp = self
            .agent
            .get(url)
            .config()
            .timeout_global(Some(LONG))
            .build()
            .call()?;
        let status = resp.status().as_u16();
        if status != 200 {
            return Err(HttpError::Status(status));
        }
        Ok(resp
            .body_mut()
            .with_config()
            .limit(MAX_IMAGE_BYTES as u64)
            .read_to_vec()?)
    }
}

impl Http {
    fn get_authed(
        &self,
        path: &str,
        token: &SessionToken,
        from: usize,
        limit: u64,
    ) -> Result<(u16, Vec<u8>), HttpError> {
        let mut req = self
            .agent
            .get(format!("{}{path}", self.base))
            .header(PROTOCOL_HEADER, PROTOCOL_VERSION.to_string())
            .header("authorization", format!("Bearer {}", token.to_hex()));
        if from > 0 {
            req = req.header("range", format!("bytes={from}-"));
        }
        let mut resp = req.config().timeout_global(Some(LONG)).build().call()?;
        let status = resp.status().as_u16();
        let bytes = resp.body_mut().with_config().limit(limit).read_to_vec()?;
        Ok((status, bytes))
    }

    pub fn compendium_index(&self, token: &SessionToken) -> Result<Vec<SourceInfo>, HttpError> {
        let (status, body) = self.get_authed("/api/compendium", token, 0, SMALL_BODY)?;
        if status != 200 {
            return Err(HttpError::Status(status));
        }
        postcard::from_bytes(&body).map_err(|_| HttpError::Decode)
    }
    pub fn library_changes(
        &self,
        token: &SessionToken,
        after: u64,
    ) -> Result<LibraryChanges, HttpError> {
        let path = format!("/api/library/changes/{after}");
        let (status, body) = self.get_authed(&path, token, 0, MAX_BUNDLE_BYTES as u64)?;
        if status != 200 {
            return Err(HttpError::Status(status));
        }
        postcard::from_bytes(&body).map_err(|_| HttpError::Decode)
    }

    pub fn library_imports(&self, token: &SessionToken) -> Result<Vec<ImportInfo>, HttpError> {
        let (status, body) = self.get_authed("/api/library/imports", token, 0, SMALL_BODY)?;
        if status != 200 {
            return Err(HttpError::Status(status));
        }
        postcard::from_bytes(&body).map_err(|_| HttpError::Decode)
    }

    pub fn bundle_from(
        &self,
        token: &SessionToken,
        sha256: &str,
        from: usize,
    ) -> Result<BundlePart, HttpError> {
        let path = format!("/api/compendium/{sha256}");
        match self.get_authed(&path, token, from, MAX_BUNDLE_BYTES as u64)? {
            (200, bytes) => Ok(BundlePart::Whole(bytes)),
            (206, bytes) if from > 0 => Ok(BundlePart::Rest(bytes)),
            (status, _) => Err(HttpError::Status(status)),
        }
    }
}

pub enum BundlePart {
    Whole(Vec<u8>),
    Rest(Vec<u8>),
}

fn decode_or_error<T: DeserializeOwned, E: DeserializeOwned>(
    status: u16,
    body: &[u8],
    wrap: impl FnOnce(E) -> HttpError,
) -> Result<T, HttpError> {
    if status == 200 {
        return postcard::from_bytes(body).map_err(|_| HttpError::Decode);
    }
    match postcard::from_bytes::<E>(body) {
        Ok(e) => Err(wrap(e)),
        Err(_) => Err(HttpError::Status(status)),
    }
}
