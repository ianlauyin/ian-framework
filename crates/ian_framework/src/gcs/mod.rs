//! Google Cloud Storage client.
//!
//! Build a backend with [`gcs_from_adc`] or [`gcs_backend`].
//! The default crate features do not compile this module.
//! Select the `gcs` Cargo feature to compile it.
//!
//! This module is a bucket client. It does not include a product store,
//! trash rules, browse limits, or an HTTP framework.

mod cred;
mod encode;
mod object;
mod signed_url;
mod token;

pub use cred::Auth;
pub use encode::{encode_object_id, encode_object_path, encode_seg};
pub use object::{
    byte_stream, copy_object, delete_object, get_bytes, list, open_object, patch_metadata,
    put_bytes, put_file, put_stream, read_to_vec, rewrite_object, stat, ListPage, ListQuery,
    ObjectMeta, ObjectReader, PutOptions,
};
pub use signed_url::{sign_blob, signed_get_url};

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::gcs::token::CachedToken;

/// Limit for one resumable chunk, one session probe, and unacknowledged TCP data.
///
/// The HTTP client sets `TCP_USER_TIMEOUT` to this value on Linux.
/// A resumable chunk request uses this same limit.
/// A session probe uses this same limit.
pub const TRANSFER_TIMEOUT: Duration = Duration::from_secs(180);

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Bucket client.
///
/// Clone shares the HTTP client and the access-token cache.
#[derive(Clone)]
pub struct GcsBackend {
    inner: Arc<Inner>,
}

struct Inner {
    bucket: String,
    email: String,
    auth: Auth,
    http: reqwest::Client,
    token: tokio::sync::Mutex<Option<CachedToken>>,
}

/// Why a GCS call failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcsErrorKind {
    Auth,
    Config,
    NotFound,
    Api,
    Transport,
}

/// Error from the GCS client.
pub struct GcsError {
    kind: GcsErrorKind,
    status: Option<u16>,
    message: String,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl GcsError {
    pub fn kind(&self) -> GcsErrorKind {
        self.kind
    }

    pub(crate) fn new(kind: GcsErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            status: None,
            message: message.into(),
            source: None,
        }
    }

    pub(crate) fn with_status(kind: GcsErrorKind, status: u16, message: impl Into<String>) -> Self {
        Self {
            kind,
            status: Some(status),
            message: message.into(),
            source: None,
        }
    }

    pub(crate) fn with_source(
        kind: GcsErrorKind,
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            status: None,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
}

impl fmt::Debug for GcsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GcsError")
            .field("kind", &self.kind)
            .field("status", &self.status)
            .field("message", &self.message)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for GcsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for GcsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.source {
            Some(source) => Some(source.as_ref()),
            None => None,
        }
    }
}

impl fmt::Debug for GcsBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GcsBackend")
            .field("bucket", &self.inner.bucket)
            .field("email", &self.inner.email)
            .field("auth", &self.inner.auth)
            .finish_non_exhaustive()
    }
}

/// Bucket name stored on the backend.
pub fn bucket(backend: &GcsBackend) -> &str {
    &backend.inner.bucket
}

/// Service account email used for V4 signatures.
pub fn signer_email(backend: &GcsBackend) -> &str {
    &backend.inner.email
}

/// Build a backend from an explicit auth value.
///
/// `Auth::Metadata` reads the VM service account email from the metadata server.
/// `Auth::User` does not contact the network during construction.
///
/// ```no_run
/// # async fn demo() -> Result<(), ian_framework::gcs::GcsError> {
/// use ian_framework::gcs::{self, Auth};
/// let backend = gcs::gcs_backend("my-bucket", Auth::Metadata).await?;
/// let _ = gcs::signer_email(&backend);
/// # Ok(())
/// # }
/// ```
pub async fn gcs_backend(
    bucket_name: impl Into<String>,
    auth: Auth,
) -> Result<GcsBackend, GcsError> {
    let bucket_name = bucket_name.into();
    validate_bucket(&bucket_name)?;
    cred::validate_auth(&auth)?;
    let http = http_client()?;
    let email = match &auth {
        Auth::Metadata => token::metadata_email(&http).await?,
        Auth::User { signer_email, .. } => signer_email.clone(),
    };
    Ok(GcsBackend {
        inner: Arc::new(Inner {
            bucket: bucket_name,
            email,
            auth,
            http,
            token: tokio::sync::Mutex::new(None),
        }),
    })
}

/// Build a backend from Application Default Credentials.
///
/// Order:
///
/// 1. `GOOGLE_APPLICATION_CREDENTIALS`, when the variable is set and not empty.
/// 2. The well-known `application_default_credentials.json` file.
/// 3. The VM metadata server.
///
/// A service account JSON key is an error. Impersonated ADC is an error.
/// User ADC requires `GCS_SIGNER_EMAIL`.
///
/// ```no_run
/// # async fn demo() -> Result<(), ian_framework::gcs::GcsError> {
/// let backend = ian_framework::gcs::gcs_from_adc("my-bucket").await?;
/// let _ = ian_framework::gcs::bucket(&backend);
/// # Ok(())
/// # }
/// ```
pub async fn gcs_from_adc(bucket_name: impl Into<String>) -> Result<GcsBackend, GcsError> {
    let auth = match cred::load_adc_auth().await? {
        Some(auth) => auth,
        None => Auth::Metadata,
    };
    gcs_backend(bucket_name, auth).await
}

pub(crate) fn http(backend: &GcsBackend) -> &reqwest::Client {
    &backend.inner.http
}

pub(crate) fn auth(backend: &GcsBackend) -> &Auth {
    &backend.inner.auth
}

pub(crate) fn token_cache(backend: &GcsBackend) -> &tokio::sync::Mutex<Option<CachedToken>> {
    &backend.inner.token
}

fn validate_bucket(bucket_name: &str) -> Result<(), GcsError> {
    if bucket_name.is_empty()
        || bucket_name.contains(['/', '\\', ' ', '\r', '\n'])
        || !bucket_name.is_ascii()
    {
        return Err(GcsError::new(
            GcsErrorKind::Config,
            "bucket name is empty or contains a forbidden character",
        ));
    }
    Ok(())
}

fn http_client() -> Result<reqwest::Client, GcsError> {
    let builder = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("ian_framework/", env!("CARGO_PKG_VERSION")));
    #[cfg(target_os = "linux")]
    let builder = builder.tcp_user_timeout(TRANSFER_TIMEOUT);
    builder.build().map_err(|err| {
        GcsError::with_source(GcsErrorKind::Transport, "cannot build the HTTP client", err)
    })
}

pub(crate) struct Call {
    pub method: reqwest::Method,
    pub url: String,
    pub headers: Vec<(reqwest::header::HeaderName, String)>,
    pub json: Option<serde_json::Value>,
    pub bytes: Option<Vec<u8>>,
    /// Status codes that are success in addition to 2xx. `308` is used by resumable upload.
    pub extra_status: &'static [u16],
    /// Total time for this request. `None` does not add a request timeout.
    pub timeout: Option<Duration>,
}

pub(crate) fn is_retryable(err: &GcsError) -> bool {
    if err.kind == GcsErrorKind::Transport {
        return true;
    }
    matches!(err.status, Some(408 | 429 | 500 | 502 | 503 | 504))
}

pub(crate) async fn send(backend: &GcsBackend, call: &Call) -> Result<reqwest::Response, GcsError> {
    let mut retried = false;
    loop {
        let token = token::access_token(backend).await?;
        let mut request = http(backend).request(call.method.clone(), &call.url);
        request = request.bearer_auth(&token);
        for (name, value) in &call.headers {
            request = request.header(name, value);
        }
        if let Some(timeout) = call.timeout {
            request = request.timeout(timeout);
        }
        if let Some(json) = &call.json {
            request = request.json(json);
        } else if let Some(bytes) = &call.bytes {
            request = request.body(bytes.clone());
        }
        let response = request.send().await.map_err(|err| {
            GcsError::with_source(GcsErrorKind::Transport, "GCS request failed", err)
        })?;
        let status = response.status().as_u16();
        tracing::debug!(method = %call.method, url = %call.url, status, "gcs response");
        if status == 401 && !retried {
            retried = true;
            let _ = response.bytes().await;
            token::invalidate(backend).await;
            tracing::debug!("gcs access token rejected; refreshing");
            continue;
        }
        if response.status().is_success() || call.extra_status.contains(&status) {
            return Ok(response);
        }
        return Err(error_from_response(response).await);
    }
}

async fn error_from_response(response: reqwest::Response) -> GcsError {
    let status = response.status().as_u16();
    let text = response.text().await.unwrap_or_default();
    let detail = api_detail(&text).unwrap_or(text);
    let detail = truncate(&detail, 500);
    let kind = match status {
        401 | 403 => GcsErrorKind::Auth,
        404 => GcsErrorKind::NotFound,
        _ => GcsErrorKind::Api,
    };
    GcsError::with_status(kind, status, format!("status {status}: {detail}"))
}

fn api_detail(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    if let Some(message) = value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(|m| m.as_str())
    {
        return Some(message.to_string());
    }
    if let Some(message) = value.get("error_description").and_then(|m| m.as_str()) {
        return Some(message.to_string());
    }
    None
}

pub(crate) fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_auth() -> Auth {
        Auth::User {
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            refresh_token: "refresh".to_string(),
            signer_email: "signer@example.iam.gserviceaccount.com".to_string(),
        }
    }

    #[tokio::test]
    async fn user_backend_keeps_bucket_and_signer() {
        let backend = gcs_backend("my-bucket", user_auth()).await.unwrap();
        assert_eq!(bucket(&backend), "my-bucket");
        assert_eq!(
            signer_email(&backend),
            "signer@example.iam.gserviceaccount.com"
        );
        let cloned = backend.clone();
        assert_eq!(bucket(&cloned), "my-bucket");
    }

    #[tokio::test]
    async fn backend_rejects_an_empty_bucket() {
        let err = gcs_backend(" ", user_auth()).await.unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Config);
    }

    #[tokio::test]
    async fn backend_rejects_an_incomplete_user() {
        let auth = Auth::User {
            client_id: String::new(),
            client_secret: "secret".to_string(),
            refresh_token: "refresh".to_string(),
            signer_email: "signer@example.iam.gserviceaccount.com".to_string(),
        };
        let err = gcs_backend("my-bucket", auth).await.unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Config);
    }

    #[test]
    fn api_detail_reads_the_gcs_message() {
        let body = r#"{"error":{"code":404,"message":"No such object"}}"#;
        assert_eq!(api_detail(body).as_deref(), Some("No such object"));
    }

    #[test]
    fn transfer_timeout_is_180_seconds() {
        assert_eq!(TRANSFER_TIMEOUT, Duration::from_secs(180));
    }

    #[test]
    fn retryable_status_codes_match_the_upload_policy() {
        assert!(is_retryable(&GcsError::new(
            GcsErrorKind::Transport,
            "reset"
        )));
        for status in [408_u16, 429, 500, 502, 503, 504] {
            assert!(is_retryable(&GcsError::with_status(
                GcsErrorKind::Api,
                status,
                "retry"
            )));
        }
        assert!(!is_retryable(&GcsError::with_status(
            GcsErrorKind::NotFound,
            404,
            "missing"
        )));
        assert!(!is_retryable(&GcsError::with_status(
            GcsErrorKind::Api,
            400,
            "bad"
        )));
        assert!(!is_retryable(&GcsError::new(
            GcsErrorKind::Config,
            "config"
        )));
    }
}
