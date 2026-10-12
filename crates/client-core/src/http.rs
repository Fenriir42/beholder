//! Blocking HTTPS calls (auth and images). Call them from a blocking thread
//! (`tokio::task::spawn_blocking`), never from the UI thread.

use std::time::Duration;

use beholder_core::ImageHash;
use beholder_protocol::http::{
    AuthError, AuthResponse, ImageError, ImageUploaded, LoginRequest, MAX_IMAGE_BYTES,
    RegisterRequest,
};
use beholder_protocol::{PROTOCOL_HEADER, PROTOCOL_VERSION, SessionToken};
use serde::Serialize;
use serde::de::DeserializeOwned;
use ureq::Agent;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

/// Short calls (auth): a slow VPN still answers within this.
const SHORT: Duration = Duration::from_secs(30);

/// Image transfers: 20 MB at ~30 KB/s takes about 11 minutes.
const LONG: Duration = Duration::from_secs(15 * 60);

/// Largest small response accepted.
const SMALL_BODY: u64 = 64 * 1024;

/// Which stored size of an image to download.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageVariant {
    /// ~480 px wide.
    Thumb,
    /// Up to 1920 px.
    Display,
}

/// Why an HTTP call failed.
#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    /// The server refused the login or registration.
    #[error("auth: {0:?}")]
    Auth(AuthError),
    /// The server refused the image.
    #[error("image: {0:?}")]
    Image(ImageError),
    /// Unexpected HTTP status.
    #[error("HTTP {0}")]
    Status(u16),
    /// Couldn't reach the server, or the transfer failed.
    #[error("network: {0}")]
    Network(String),
    /// The response body wasn't what we expected.
    #[error("bad response")]
    Decode,
}

impl From<ureq::Error> for HttpError {
    fn from(e: ureq::Error) -> Self {
        Self::Network(e.to_string())
    }
}

/// HTTPS client for one server.
pub struct Http {
    agent: Agent,
    base: String,
}

impl Http {
    /// A client for `server_url` (e.g. `https://dnd.example.org`).
    pub fn new(server_url: &str) -> Self {
        let config = Agent::config_builder()
            // Error statuses carry postcard bodies we want to read.
            .http_status_as_error(false)
            .timeout_global(Some(SHORT))
            .tls_config(
                TlsConfig::builder()
                    .provider(TlsProvider::NativeTls)
                    // The OS certificate store, so no root bundle ships in the binary.
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

    /// Creates an account and logs in.
    pub fn register(&self, req: &RegisterRequest) -> Result<AuthResponse, HttpError> {
        self.auth("/api/register", req)
    }

    /// Logs in.
    pub fn login(&self, req: &LoginRequest) -> Result<AuthResponse, HttpError> {
        self.auth("/api/login", req)
    }

    /// Ends the session on the server.
    pub fn logout(&self, token: &SessionToken) -> Result<(), HttpError> {
        let (status, _) = self.post("/api/logout", Some(token), &[], false, SMALL_BODY)?;
        if (200..300).contains(&status) {
            Ok(())
        } else {
            Err(HttpError::Status(status))
        }
    }

    /// Uploads an image; the server re-encodes it and returns its hash.
    pub fn upload_image(&self, token: &SessionToken, bytes: &[u8]) -> Result<ImageHash, HttpError> {
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(HttpError::Image(ImageError::TooLarge));
        }
        let (status, body) = self.post("/api/images", Some(token), bytes, true, SMALL_BODY)?;
        decode_or_error::<ImageUploaded, _>(status, &body, HttpError::Image).map(|u| u.hash)
    }

    /// Downloads one stored image variant (WebP bytes).
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
