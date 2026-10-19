mod cred;
mod encode;
mod object;
mod ops;
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
use std::time::{Duration, Instant};

use crate::gcs::token::CachedToken;

pub const TRANSFER_TIMEOUT: Duration = Duration::from_secs(180);

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

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
    ops_log: std::sync::Mutex<Option<crate::logging::LogShipper>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcsErrorKind {
    Auth,
    Config,
    NotFound,
    Api,
    Transport,
}

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

    pub(crate) fn status(&self) -> Option<u16> {
        self.status
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

pub fn bucket(backend: &GcsBackend) -> &str {
    &backend.inner.bucket
}

pub fn signer_email(backend: &GcsBackend) -> &str {
    &backend.inner.email
}

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
            ops_log: std::sync::Mutex::new(None),
        }),
    })
}

pub fn attach_ops_log(backend: &GcsBackend, shipper: crate::logging::LogShipper) {
    *lock_ops(&backend.inner.ops_log) = Some(shipper);
}

pub(crate) fn ops_shipper(backend: &GcsBackend) -> Option<crate::logging::LogShipper> {
    lock_ops(&backend.inner.ops_log).clone()
}

fn lock_ops(
    slot: &std::sync::Mutex<Option<crate::logging::LogShipper>>,
) -> std::sync::MutexGuard<'_, Option<crate::logging::LogShipper>> {
    slot.lock().unwrap_or_else(|err| err.into_inner())
}

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

    pub extra_status: &'static [u16],

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
        let started = Instant::now();
        let response = match request.send().await {
            Ok(response) => response,
            Err(err) => {
                let error_text = ops::logged_error(&err);
                ops::record_op(
                    backend,
                    ops::OpsRecord {
                        level: crate::logging::LogLevel::Warn,
                        message: "gcs request failed",
                        method: call.method.as_str(),
                        url: &call.url,
                        status: None,
                        latency_ms: Some(ops::millis_since(started)),
                        extra: &[("error", &error_text)],
                    },
                );
                return Err(GcsError::with_source(
                    GcsErrorKind::Transport,
                    "GCS request failed",
                    err,
                ));
            }
        };
        let status = response.status().as_u16();
        let latency_ms = ops::millis_since(started);
        if status == 401 && !retried {
            retried = true;
            let _ = response.bytes().await;
            token::invalidate(backend).await;
            ops::record_op(
                backend,
                ops::OpsRecord {
                    level: crate::logging::LogLevel::Warn,
                    message: "gcs access token rejected",
                    method: call.method.as_str(),
                    url: &call.url,
                    status: Some(status),
                    latency_ms: Some(latency_ms),
                    extra: &[],
                },
            );
            continue;
        }
        ops::record_op(
            backend,
            ops::OpsRecord {
                level: ops::level_for_status(status),
                message: "gcs response",
                method: call.method.as_str(),
                url: &call.url,
                status: Some(status),
                latency_ms: Some(latency_ms),
                extra: &[],
            },
        );
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

    #[tokio::test]
    async fn ops_log_writes_the_bucket_and_omits_the_query() {
        let backend = gcs_backend("my-bucket", user_auth()).await.unwrap();
        let mut config = crate::logging::shipper_config("storage");
        config.project = "demo".to_string();
        config.minimum_level = crate::logging::LogLevel::Debug;
        let shipper = crate::logging::log_shipper(config).unwrap();
        attach_ops_log(&backend, shipper.clone());
        ops::record_op(
            &backend,
            ops::OpsRecord {
                level: crate::logging::LogLevel::Debug,
                message: "gcs response",
                method: "GET",
                url:
                    "https://storage.googleapis.com/storage/v1/b/my-bucket/o/file?upload_id=secret",
                status: Some(200),
                latency_ms: Some(3),
                extra: &[],
            },
        );
        let events = crate::logging::buffered_events(&shipper);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "GET /storage/v1/b/my-bucket/o/file");
        assert_eq!(events[0].id, "");
        assert_eq!(events[0].ip, "");
        assert_eq!(events[0].user_agent, "");
        assert_eq!(events[0].status, Some(200));
        assert_eq!(events[0].latency_ms, Some(3));
        assert_eq!(
            events[0]
                .extra
                .get("bucket")
                .and_then(|value| value.as_str()),
            Some("my-bucket")
        );
        let line = serde_json::to_string(&events[0]).unwrap();
        let value: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert!(value.get("app").is_none());
        assert!(value.get("project").is_none());
        assert!(!line.contains("secret"));
        assert!(!line.contains("upload_id"));
    }

    #[tokio::test]
    async fn ops_debug_is_hidden_at_the_default_minimum() {
        let backend = gcs_backend("my-bucket", user_auth()).await.unwrap();
        let shipper =
            crate::logging::log_shipper(crate::logging::shipper_config("storage")).unwrap();
        attach_ops_log(&backend, shipper.clone());
        ops::record_op(
            &backend,
            ops::OpsRecord {
                level: crate::logging::LogLevel::Debug,
                message: "gcs response",
                method: "GET",
                url: "https://storage.googleapis.com/storage/v1/b/my-bucket/o/file",
                status: Some(200),
                latency_ms: Some(1),
                extra: &[],
            },
        );
        assert_eq!(crate::logging::buffered_len(&shipper), 0);
        ops::record_op(
            &backend,
            ops::OpsRecord {
                level: crate::logging::LogLevel::Warn,
                message: "gcs request failed",
                method: "GET",
                url: "https://storage.googleapis.com/storage/v1/b/my-bucket/o/file",
                status: None,
                latency_ms: None,
                extra: &[],
            },
        );
        assert_eq!(crate::logging::buffered_len(&shipper), 1);
        assert_eq!(
            crate::logging::buffered_events(&shipper)[0].message,
            "gcs request failed"
        );
    }

    #[tokio::test]
    async fn ops_log_without_a_shipper_does_not_panic() {
        let backend = gcs_backend("my-bucket", user_auth()).await.unwrap();
        ops::record_op(
            &backend,
            ops::OpsRecord {
                level: crate::logging::LogLevel::Warn,
                message: "gcs request failed",
                method: "GET",
                url: "https://storage.googleapis.com/storage/v1/b/my-bucket/o/file",
                status: None,
                latency_ms: None,
                extra: &[],
            },
        );
    }
}
