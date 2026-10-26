mod auth;
mod call;
mod list_page;
mod list_query;
mod object_meta;
mod object_reader;
mod ops_record;
mod put_request;
mod rewrite_step;
mod signed_parts;
mod upload_source;
mod upload_step;

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use chrono::Utc;

use crate::logging::{LogEvent, LogLevel, LogShipper};

use super::encode::{encode_object_id, encode_seg};
use super::{GcsError, GcsErrorKind, API_ROOT};

use auth::{Auth, CachedToken};
use call::Call;
use ops_record::OpsRecord;
use rewrite_step::RewriteStep;
use signed_parts::SignedParts;
use upload_source::UploadSource;
use upload_step::UploadStep;

pub use list_page::ListPage;
pub use list_query::ListQuery;
pub use object_meta::ObjectMeta;
pub use object_reader::ObjectReader;
pub use put_request::{PutBody, PutOptions, PutRequest};

const TRANSFER_TIMEOUT: Duration = Duration::from_secs(180);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CHUNK_SIZE: usize = 8 * 1024 * 1024;
const CHUNK_ALIGN: u64 = 256 * 1024;
const CHUNK_ATTEMPTS: u32 = 5;
const CHUNK_BACKOFF: Duration = Duration::from_millis(200);
const UPLOAD_ROOT: &str = "https://storage.googleapis.com/upload/storage/v1";

#[derive(Clone)]
pub struct GcsClient {
    inner: Arc<Inner>,
}

impl GcsClient {
    pub fn bucket(&self) -> &str {
        &self.inner.bucket
    }

    pub fn signer_email(&self) -> &str {
        &self.inner.email
    }

    pub async fn from_adc(bucket_name: impl Into<String>) -> Result<Self, GcsError> {
        Self::new(bucket_name, Auth::from_adc().await?).await
    }

    pub async fn get_bytes(&self, name: &str) -> Result<Vec<u8>, GcsError> {
        self.open_object(name).await?.read_to_vec().await
    }

    pub async fn get_bytes_opt(&self, name: &str) -> Result<Option<Vec<u8>>, GcsError> {
        not_found_as_none(self.get_bytes(name).await)
    }

    pub async fn open_object(&self, name: &str) -> Result<ObjectReader, GcsError> {
        validate_object_name(name)?;
        let response = self
            .send(&Call {
                method: reqwest::Method::GET,
                url: format!("{}?alt=media", object_url(self.bucket(), name)),
                ..Default::default()
            })
            .await?;
        Ok(ObjectReader { response })
    }

    pub async fn open_object_opt(&self, name: &str) -> Result<Option<ObjectReader>, GcsError> {
        not_found_as_none(self.open_object(name).await)
    }

    pub async fn stat(&self, name: &str) -> Result<ObjectMeta, GcsError> {
        validate_object_name(name)?;
        let response = self
            .send(&Call {
                method: reqwest::Method::GET,
                url: object_url(self.bucket(), name),
                ..Default::default()
            })
            .await?;
        ObjectMeta::from_response(response).await
    }

    pub async fn stat_opt(&self, name: &str) -> Result<Option<ObjectMeta>, GcsError> {
        not_found_as_none(self.stat(name).await)
    }

    pub async fn list(&self, query: &ListQuery) -> Result<ListPage, GcsError> {
        let response = self
            .send(&Call {
                method: reqwest::Method::GET,
                url: query.url(self.bucket()),
                ..Default::default()
            })
            .await?;
        let bytes = response.bytes().await.map_err(|err| {
            GcsError::with_source(GcsErrorKind::Transport, "list response is not bytes", err)
        })?;
        ListPage::from_slice(&bytes)
    }

    pub async fn put(&self, request: &PutRequest) -> Result<ObjectMeta, GcsError> {
        let name = request.name.as_str();
        let options = &request.options;
        validate_object_name(name)?;
        let content_type = options.content_type()?;
        validate_metadata(&options.metadata)?;
        let mut source = UploadSource::open(&request.body).await?;
        let total = source.len();
        let location = self
            .start_resumable(name, &content_type, &options.metadata, total)
            .await?;
        if total == 0 {
            return self
                .put_chunk_retrying(&location, &content_type, b"", 0, 0, true)
                .await?
                .expect_done();
        }
        let mut offset = 0u64;
        let mut rewinds = 0u32;
        while offset < total {
            let remaining = total - offset;
            let is_last = remaining <= CHUNK_SIZE as u64;
            let len = usize::try_from(remaining.min(CHUNK_SIZE as u64)).unwrap_or(CHUNK_SIZE);
            let chunk = source.read_chunk(offset, len).await?;
            match self
                .put_chunk_retrying(&location, &content_type, &chunk, offset, total, is_last)
                .await?
            {
                UploadStep::Done(meta) => return Ok(meta),
                UploadStep::Continue { next } => {
                    advance_offset(&mut offset, &mut rewinds, next, total)?;
                }
            }
        }
        Err(GcsError::new(
            GcsErrorKind::Api,
            "finished upload did not return object metadata",
        ))
    }

    pub async fn rewrite_object(&self, from: &str, to: &str) -> Result<ObjectMeta, GcsError> {
        validate_object_name(from)?;
        validate_object_name(to)?;
        let mut token: Option<String> = None;
        loop {
            let response = self
                .send(&Call {
                    method: reqwest::Method::POST,
                    url: RewriteStep::url(self.bucket(), from, to, token.as_deref()),
                    json: Some(serde_json::json!({})),
                    ..Default::default()
                })
                .await?;
            let bytes = response.bytes().await.map_err(|err| {
                GcsError::with_source(
                    GcsErrorKind::Transport,
                    "rewrite response is not bytes",
                    err,
                )
            })?;
            let next = match RewriteStep::from_slice(&bytes)? {
                RewriteStep::Done(meta) => return Ok(meta),
                RewriteStep::Next(next) => next,
            };
            if token.as_deref() == Some(next.as_str()) {
                return Err(GcsError::new(
                    GcsErrorKind::Api,
                    "rewrite token did not change",
                ));
            }
            token = Some(next);
        }
    }

    pub async fn patch_metadata(
        &self,
        name: &str,
        metadata: &BTreeMap<String, String>,
    ) -> Result<ObjectMeta, GcsError> {
        validate_object_name(name)?;
        validate_metadata(metadata)?;
        let response = self
            .send(&Call {
                method: reqwest::Method::PATCH,
                url: object_url(self.bucket(), name),
                json: Some(serde_json::json!({ "metadata": metadata })),
                ..Default::default()
            })
            .await?;
        ObjectMeta::from_response(response).await
    }

    pub async fn delete_object(&self, name: &str) -> Result<(), GcsError> {
        validate_object_name(name)?;
        let result = self
            .send(&Call {
                method: reqwest::Method::DELETE,
                url: object_url(self.bucket(), name),
                ..Default::default()
            })
            .await;
        let response = match result {
            Ok(response) => response,
            Err(err) if err.kind() == GcsErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err),
        };
        let _ = response.bytes().await;
        Ok(())
    }

    pub async fn move_object(&self, from: &str, to: &str) -> Result<ObjectMeta, GcsError> {
        if from == to {
            return Err(GcsError::new(
                GcsErrorKind::Config,
                "move source and destination are the same object",
            ));
        }
        let meta = self.rewrite_object(from, to).await?;
        if let Err(err) = self.delete_object(from).await {
            // Without this the object would exist at both names.
            let _ = self.delete_object(to).await;
            return Err(err);
        }
        Ok(meta)
    }

    pub async fn signed_get_url(
        &self,
        name: &str,
        expires_in: Option<Duration>,
        attachment_filename: Option<&str>,
    ) -> Result<String, GcsError> {
        validate_object_name(name)?;
        let parts = SignedParts::v4_get(
            self.bucket(),
            name,
            self.signer_email(),
            Utc::now(),
            expires_in,
            attachment_filename,
        )?;
        let signature = self.sign_blob(parts.string_to_sign.as_bytes()).await?;
        Ok(format!(
            "{}&X-Goog-Signature={}",
            parts.url_prefix,
            hex::encode(signature)
        ))
    }

    pub fn attach_ops_log(&self, shipper: LogShipper) {
        *lock_ops(&self.inner.ops_log) = Some(shipper);
    }

    async fn new(bucket_name: impl Into<String>, auth: Auth) -> Result<Self, GcsError> {
        let bucket_name = bucket_name.into();
        validate_bucket(&bucket_name)?;
        auth.validate()?;
        let http = http_client()?;
        let email = auth.signer_email(&http).await?;
        Ok(Self {
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

    async fn start_resumable(
        &self,
        name: &str,
        content_type: &str,
        metadata: &BTreeMap<String, String>,
        total_len: u64,
    ) -> Result<String, GcsError> {
        let url = format!(
            "{UPLOAD_ROOT}/b/{}/o?uploadType=resumable",
            encode_seg(self.bucket()),
        );
        let mut body = serde_json::json!({
            "name": name,
            "contentType": content_type,
        });
        if !metadata.is_empty() {
            body["metadata"] = serde_json::json!(metadata);
        }
        let headers = vec![
            (
                reqwest::header::HeaderName::from_static("x-upload-content-type"),
                content_type.to_string(),
            ),
            (
                reqwest::header::HeaderName::from_static("x-upload-content-length"),
                total_len.to_string(),
            ),
        ];
        let response = self
            .send(&Call {
                method: reqwest::Method::POST,
                url,
                headers,
                json: Some(body),
                ..Default::default()
            })
            .await?;
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .ok_or_else(|| {
                GcsError::new(
                    GcsErrorKind::Api,
                    "resumable upload did not return Location",
                )
            })?;
        let location = location
            .to_str()
            .map_err(|_| {
                GcsError::new(GcsErrorKind::Api, "resumable Location header is not UTF-8")
            })?
            .to_string();
        let _ = response.bytes().await;
        if !location.starts_with("https://") {
            return Err(GcsError::new(
                GcsErrorKind::Api,
                "resumable Location is not an https URL",
            ));
        }
        Ok(location)
    }

    async fn put_chunk_retrying(
        &self,
        location: &str,
        content_type: &str,
        chunk: &[u8],
        start: u64,
        total: u64,
        finished: bool,
    ) -> Result<UploadStep, GcsError> {
        let chunk_len = chunk.len() as u64;
        let mut offset = start;
        let mut attempt = 0u32;
        loop {
            if chunk_suffix_len(chunk_len, start, offset, finished).is_none() {
                return Ok(UploadStep::Continue { next: offset });
            }
            attempt += 1;
            if attempt > CHUNK_ATTEMPTS {
                return Err(GcsError::new(
                    GcsErrorKind::Transport,
                    "resumable chunk failed after retries",
                ));
            }
            if attempt > 1 {
                tokio::time::sleep(retry_delay(attempt - 1)).await;
            }
            let piece = &chunk[(offset - start) as usize..];
            let sent = self
                .upload_chunk_once(location, content_type, piece, offset, total)
                .await;
            match sent {
                Ok(UploadStep::Done(meta)) => return Ok(UploadStep::Done(meta)),
                Ok(UploadStep::Continue { next }) => {
                    if next >= start + chunk_len {
                        return Ok(UploadStep::Continue { next });
                    }
                    if next > offset {
                        offset = next;
                        attempt = 0;
                        continue;
                    }
                }
                Err(err) if !err.is_retryable() => return Err(err),
                Err(err) => {
                    let attempt_text = attempt.to_string();
                    let offset_text = offset.to_string();
                    let error_text = logged_error(&err);
                    self.record_op(OpsRecord {
                        level: LogLevel::Warn,
                        message: "gcs resumable chunk failed",
                        method: "PUT",
                        url: location,
                        status: err.status(),
                        latency_ms: None,
                        extra: &[
                            ("attempt", &attempt_text),
                            ("offset", &offset_text),
                            ("error", &error_text),
                        ],
                    });
                }
            }
            match self.probe_session(location, total).await {
                Ok(UploadStep::Done(meta)) => return Ok(UploadStep::Done(meta)),
                Ok(UploadStep::Continue { next }) => {
                    if next < start || next >= start + chunk_len {
                        return Ok(UploadStep::Continue { next });
                    }
                    if next > offset {
                        offset = next;
                        attempt = 0;
                        continue;
                    }
                    if next < offset {
                        offset = next;
                    }
                }
                Err(err) if !err.is_retryable() => return Err(err),
                Err(err) => {
                    let attempt_text = attempt.to_string();
                    let offset_text = offset.to_string();
                    let error_text = logged_error(&err);
                    self.record_op(OpsRecord {
                        level: LogLevel::Warn,
                        message: "gcs resumable probe failed",
                        method: "PUT",
                        url: location,
                        status: err.status(),
                        latency_ms: None,
                        extra: &[
                            ("attempt", &attempt_text),
                            ("offset", &offset_text),
                            ("error", &error_text),
                        ],
                    });
                }
            }
        }
    }

    async fn upload_chunk_once(
        &self,
        location: &str,
        content_type: &str,
        chunk: &[u8],
        offset: u64,
        total: u64,
    ) -> Result<UploadStep, GcsError> {
        let range = chunk_content_range(offset, chunk.len() as u64, total);
        let response = self
            .send(&Call {
                method: reqwest::Method::PUT,
                url: location.to_string(),
                headers: vec![
                    (reqwest::header::CONTENT_TYPE, content_type.to_string()),
                    (reqwest::header::CONTENT_RANGE, range),
                    (reqwest::header::CONTENT_LENGTH, chunk.len().to_string()),
                ],
                bytes: Some(chunk.to_vec()),
                extra_status: &[308],
                timeout: Some(TRANSFER_TIMEOUT),
                ..Default::default()
            })
            .await?;
        UploadStep::from_response(response, offset + chunk.len() as u64).await
    }

    async fn probe_session(&self, location: &str, total: u64) -> Result<UploadStep, GcsError> {
        let response = self
            .send(&Call {
                method: reqwest::Method::PUT,
                url: location.to_string(),
                headers: vec![
                    (reqwest::header::CONTENT_RANGE, probe_content_range(total)),
                    (reqwest::header::CONTENT_LENGTH, "0".to_string()),
                ],
                bytes: Some(Vec::new()),
                extra_status: &[308],
                timeout: Some(TRANSFER_TIMEOUT),
                ..Default::default()
            })
            .await?;
        UploadStep::from_response(response, 0).await
    }

    async fn sign_blob(&self, payload: &[u8]) -> Result<Vec<u8>, GcsError> {
        let url = format!(
            "https://iamcredentials.googleapis.com/v1/projects/-/serviceAccounts/{}:signBlob",
            encode_seg(self.signer_email()),
        );
        let encoded = base64::engine::general_purpose::STANDARD.encode(payload);
        let response = self
            .send(&Call {
                method: reqwest::Method::POST,
                url,
                json: Some(serde_json::json!({ "payload": encoded })),
                ..Default::default()
            })
            .await?;
        let bytes = response.bytes().await.map_err(|err| {
            GcsError::with_source(
                GcsErrorKind::Transport,
                "signBlob response is not bytes",
                err,
            )
        })?;
        let parsed: SignBlobResponse = serde_json::from_slice(&bytes).map_err(|err| {
            GcsError::with_source(GcsErrorKind::Api, "signBlob response is not JSON", err)
        })?;
        let cleaned: String = parsed
            .signed_blob
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect();
        let signature = base64::engine::general_purpose::STANDARD
            .decode(cleaned)
            .map_err(|err| {
                GcsError::with_source(GcsErrorKind::Api, "signBlob signature is not base64", err)
            })?;
        if signature.is_empty() {
            return Err(GcsError::new(
                GcsErrorKind::Api,
                "signBlob returned an empty signature",
            ));
        }
        Ok(signature)
    }

    fn record_op(&self, op: OpsRecord<'_>) {
        let Some(shipper) = lock_ops(&self.inner.ops_log).clone() else {
            return;
        };
        let mut event = LogEvent {
            ts: Utc::now(),
            id: String::new(),
            ip: String::new(),
            user_agent: String::new(),
            action: action(op.method, op.url),
            status: op.status,
            latency_ms: op.latency_ms,
            level: op.level,
            message: op.message.to_string(),
            extra: BTreeMap::new(),
        };
        for (key, value) in op.extra {
            event.extra.insert(
                (*key).to_string(),
                serde_json::Value::String((*value).to_string()),
            );
        }
        event.extra.insert(
            "bucket".to_string(),
            serde_json::Value::String(self.bucket().to_string()),
        );
        shipper.record(event);
    }

    async fn send(&self, call: &Call) -> Result<reqwest::Response, GcsError> {
        let mut retried = false;
        loop {
            let token = self.access_token().await?;
            let mut request = self.inner.http.request(call.method.clone(), &call.url);
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
                    let error_text = logged_error(&err);
                    self.record_op(OpsRecord {
                        level: LogLevel::Warn,
                        message: "gcs request failed",
                        method: call.method.as_str(),
                        url: &call.url,
                        status: None,
                        latency_ms: Some(millis_since(started)),
                        extra: &[("error", &error_text)],
                    });
                    return Err(GcsError::with_source(
                        GcsErrorKind::Transport,
                        "gcs request failed",
                        err,
                    ));
                }
            };
            let status = response.status().as_u16();
            let latency_ms = millis_since(started);
            if status == 401 && !retried {
                retried = true;
                let _ = response.bytes().await;
                self.invalidate_token().await;
                self.record_op(OpsRecord {
                    level: LogLevel::Warn,
                    message: "gcs access token rejected",
                    method: call.method.as_str(),
                    url: &call.url,
                    status: Some(status),
                    latency_ms: Some(latency_ms),
                    extra: &[],
                });
                continue;
            }
            if let Some(level) = level_for_status(status) {
                self.record_op(OpsRecord {
                    level,
                    message: "gcs response",
                    method: call.method.as_str(),
                    url: &call.url,
                    status: Some(status),
                    latency_ms: Some(latency_ms),
                    extra: &[],
                });
            }
            if response.status().is_success() || call.extra_status.contains(&status) {
                return Ok(response);
            }
            return Err(error_from_response(response).await);
        }
    }

    async fn access_token(&self) -> Result<String, GcsError> {
        let mut guard = self.inner.token.lock().await;
        if let Some(cached) = guard.as_ref() {
            if Instant::now() < cached.expires_at {
                return Ok(cached.value.clone());
            }
        }
        let fetched = self.inner.auth.fetch_token(&self.inner.http).await;
        if let Err(err) = &fetched {
            let error_text = logged_error(err);
            self.record_op(OpsRecord {
                level: LogLevel::Warn,
                message: "access token request failed",
                method: "",
                url: "",
                status: err.status(),
                latency_ms: None,
                extra: &[("error", &error_text)],
            });
        }
        let token = fetched?;
        let value = token.value.clone();
        *guard = Some(token);
        Ok(value)
    }

    async fn invalidate_token(&self) {
        *self.inner.token.lock().await = None;
    }
}

impl fmt::Debug for GcsClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GcsClient")
            .field("bucket", &self.inner.bucket)
            .field("email", &self.inner.email)
            .field("auth", &self.inner.auth)
            .finish_non_exhaustive()
    }
}

struct Inner {
    bucket: String,
    email: String,
    auth: Auth,
    http: reqwest::Client,
    token: tokio::sync::Mutex<Option<CachedToken>>,
    ops_log: std::sync::Mutex<Option<LogShipper>>,
}

fn not_found_as_none<T>(result: Result<T, GcsError>) -> Result<Option<T>, GcsError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(err) if err.kind() == GcsErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

fn lock_ops(
    slot: &std::sync::Mutex<Option<LogShipper>>,
) -> std::sync::MutexGuard<'_, Option<LogShipper>> {
    slot.lock().unwrap_or_else(|err| err.into_inner())
}

fn validate_object_name(name: &str) -> Result<(), GcsError> {
    if name.is_empty() || name.contains(['\r', '\n']) {
        return Err(GcsError::new(
            GcsErrorKind::Config,
            "object name is empty or contains a line break",
        ));
    }
    Ok(())
}

fn validate_metadata(metadata: &BTreeMap<String, String>) -> Result<(), GcsError> {
    for (key, value) in metadata {
        if key.is_empty() || key.contains(['\r', '\n']) || value.contains(['\r', '\n']) {
            return Err(GcsError::new(
                GcsErrorKind::Config,
                "metadata key is empty or metadata contains a line break",
            ));
        }
    }
    Ok(())
}

fn object_url(bucket_name: &str, name: &str) -> String {
    format!(
        "{API_ROOT}/b/{}/o/{}",
        encode_seg(bucket_name),
        encode_object_id(name),
    )
}

fn logged_error(err: &impl std::fmt::Display) -> String {
    let redacted = redact_query(&err.to_string());
    truncate(&redacted, 300).to_string()
}

fn redact_query(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut skip = false;
    for ch in text.chars() {
        if skip {
            if ch.is_whitespace() {
                skip = false;
                out.push(ch);
            }
            continue;
        }
        if ch == '?' {
            skip = true;
            continue;
        }
        out.push(ch);
    }
    out
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

fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
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

fn retry_delay(attempt: u32) -> Duration {
    CHUNK_BACKOFF.saturating_mul(u32::max(attempt, 1))
}

fn advance_offset(
    offset: &mut u64,
    rewinds: &mut u32,
    next: u64,
    total: u64,
) -> Result<(), GcsError> {
    if next > total {
        return Err(GcsError::new(
            GcsErrorKind::Api,
            "resumable offset is past the end of the object",
        ));
    }
    if next > *offset {
        *rewinds = 0;
        *offset = next;
        return Ok(());
    }
    if next < *offset {
        *rewinds += 1;
        if *rewinds > CHUNK_ATTEMPTS {
            return Err(GcsError::new(
                GcsErrorKind::Transport,
                "resumable session did not keep uploaded bytes",
            ));
        }
        *offset = next;
        return Ok(());
    }
    Err(GcsError::new(
        GcsErrorKind::Api,
        "resumable upload did not advance",
    ))
}

fn chunk_suffix_len(chunk_len: u64, start: u64, offset: u64, finished: bool) -> Option<u64> {
    if offset < start || offset > start + chunk_len {
        return None;
    }
    let suffix = start + chunk_len - offset;
    if suffix == 0 {
        if finished && chunk_len == 0 && offset == start {
            return Some(0);
        }
        return None;
    }
    if finished || suffix.is_multiple_of(CHUNK_ALIGN) {
        return Some(suffix);
    }
    None
}

fn chunk_content_range(start: u64, len: u64, total: u64) -> String {
    if len == 0 {
        return format!("bytes */{total}");
    }
    let end = start + len - 1;
    format!("bytes {start}-{end}/{total}")
}

fn probe_content_range(total: u64) -> String {
    format!("bytes */{total}")
}

#[derive(serde::Deserialize)]
struct SignBlobResponse {
    #[serde(rename = "signedBlob")]
    signed_blob: String,
}

fn action(method: &str, url: &str) -> String {
    if method.is_empty() {
        return String::new();
    }
    let path = path_only(url);
    let path = if path.is_empty() { "/" } else { path.as_str() };
    format!("{method} {path}")
}

fn path_only(url: &str) -> String {
    if let Ok(parsed) = reqwest::Url::parse(url) {
        return parsed.path().to_string();
    }
    match url.split_once('?') {
        Some((path, _)) => path.to_string(),
        None => url.to_string(),
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

fn millis_since(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn level_for_status(status: u16) -> Option<LogLevel> {
    if status >= 500 {
        Some(LogLevel::Error)
    } else if status >= 400 {
        Some(LogLevel::Warn)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::logging::ShipperConfig;

    use super::*;

    #[test]
    fn api_detail_reads_the_gcs_message() {
        let body = r#"{"error":{"code":404,"message":"No such object"}}"#;
        assert_eq!(api_detail(body).as_deref(), Some("No such object"));
    }

    #[test]
    fn object_url_encodes_slashes() {
        assert_eq!(
            object_url("buck", "a/c"),
            "https://storage.googleapis.com/storage/v1/b/buck/o/a%2Fc"
        );
    }

    #[test]
    fn logged_error_omits_the_query() {
        let text = logged_error(
            &"error sending request for url (https://storage.googleapis.com/o?upload_id=secret): connection reset",
        );
        assert!(!text.contains("secret"));
        assert!(!text.contains("upload_id"));
        assert!(text.contains("connection reset"));
    }

    fn user_auth() -> Auth {
        Auth::User {
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            refresh_token: "refresh".to_string(),
            signer_email: "signer@example.iam.gserviceaccount.com".to_string(),
        }
    }

    #[tokio::test]
    async fn user_client_keeps_bucket_and_signer() {
        let client = GcsClient::new("my-bucket", user_auth()).await.unwrap();
        assert_eq!(client.bucket(), "my-bucket");
        assert_eq!(
            client.signer_email(),
            "signer@example.iam.gserviceaccount.com"
        );
        let cloned = client.clone();
        assert_eq!(cloned.bucket(), "my-bucket");
    }

    #[tokio::test]
    async fn client_rejects_an_empty_bucket() {
        let err = GcsClient::new(" ", user_auth()).await.unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Config);
    }

    #[tokio::test]
    async fn client_rejects_an_incomplete_user() {
        let auth = Auth::User {
            client_id: String::new(),
            client_secret: "secret".to_string(),
            refresh_token: "refresh".to_string(),
            signer_email: "signer@example.iam.gserviceaccount.com".to_string(),
        };
        let err = GcsClient::new("my-bucket", auth).await.unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Config);
    }

    #[test]
    fn probe_and_chunk_ranges_match_the_upload_shape() {
        assert_eq!(probe_content_range(0), "bytes */0");
        assert_eq!(probe_content_range(12), "bytes */12");
        assert_eq!(chunk_content_range(0, 0, 0), "bytes */0");
        assert_eq!(chunk_content_range(8, 0, 8), "bytes */8");
        assert_eq!(chunk_content_range(0, 4, 4), "bytes 0-3/4");
    }

    #[test]
    fn non_final_suffix_must_be_a_multiple_of_256_kib() {
        let align = CHUNK_ALIGN;
        assert_eq!(chunk_suffix_len(align, 0, 0, false), Some(align));
        assert_eq!(chunk_suffix_len(align, 0, 1, false), None);
        assert_eq!(chunk_suffix_len(align, 0, 1, true), Some(align - 1));
        assert_eq!(chunk_suffix_len(0, 0, 0, true), Some(0));
        assert_eq!(chunk_suffix_len(0, 8, 8, true), Some(0));
        assert_eq!(chunk_suffix_len(align, 0, align, false), None);
    }

    #[test]
    fn advance_offset_rewinds_then_resets_after_progress() {
        let mut offset = 10u64;
        let mut rewinds = 0u32;
        advance_offset(&mut offset, &mut rewinds, 4, 100).unwrap();
        assert_eq!(offset, 4);
        assert_eq!(rewinds, 1);
        let stalled = advance_offset(&mut offset, &mut rewinds, 4, 100).unwrap_err();
        assert_eq!(stalled.kind(), GcsErrorKind::Api);
        advance_offset(&mut offset, &mut rewinds, 20, 100).unwrap();
        assert_eq!(offset, 20);
        assert_eq!(rewinds, 0);
        let past = advance_offset(&mut offset, &mut rewinds, 101, 100).unwrap_err();
        assert_eq!(past.kind(), GcsErrorKind::Api);
    }

    #[test]
    fn advance_offset_stops_after_repeated_rewinds() {
        let mut offset = 8u64;
        let mut rewinds = 0u32;
        for _ in 0..CHUNK_ATTEMPTS {
            advance_offset(&mut offset, &mut rewinds, 0, 100).unwrap();
            offset = 8;
        }
        let err = advance_offset(&mut offset, &mut rewinds, 0, 100).unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Transport);
    }

    #[test]
    fn chunk_retry_uses_the_transfer_timeout() {
        assert_eq!(TRANSFER_TIMEOUT, Duration::from_secs(180));
        assert_eq!(retry_delay(1), Duration::from_millis(200));
        assert_eq!(retry_delay(3), Duration::from_millis(600));
    }

    #[tokio::test]
    async fn empty_final_chunk_sends_content_length_zero() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let location = format!("http://{}/session", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut head = Vec::new();
            let mut buf = [0u8; 1024];
            while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = socket.read(&mut buf).await.unwrap();
                head.extend_from_slice(&buf[..n]);
            }
            let body = br#"{"name":"photos/","size":"0"}"#;
            let reply = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n",
                body.len()
            );
            socket.write_all(reply.as_bytes()).await.unwrap();
            socket.write_all(body).await.unwrap();
            String::from_utf8(head).unwrap().to_ascii_lowercase()
        });
        let auth = crate::gcs::gcs_client::Auth::User {
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            refresh_token: "refresh".to_string(),
            signer_email: "signer@example.iam.gserviceaccount.com".to_string(),
        };
        let client = GcsClient::new("my-bucket", auth).await.unwrap();
        *client.inner.token.lock().await = Some(crate::gcs::gcs_client::CachedToken {
            value: "token".to_string(),
            expires_at: std::time::Instant::now() + Duration::from_secs(600),
        });
        client
            .put_chunk_retrying(&location, "application/octet-stream", b"", 0, 0, true)
            .await
            .unwrap()
            .expect_done()
            .unwrap();
        let head = server.await.unwrap();
        assert!(head.contains("\r\ncontent-length: 0\r\n"), "{head}");
    }

    #[test]
    fn not_found_becomes_none() {
        let missing: Result<(), GcsError> = Err(GcsError::with_status(
            GcsErrorKind::NotFound,
            404,
            "missing",
        ));
        assert!(not_found_as_none(missing).unwrap().is_none());
        assert_eq!(not_found_as_none(Ok(7)).unwrap(), Some(7));
        let denied: Result<(), GcsError> =
            Err(GcsError::with_status(GcsErrorKind::Auth, 403, "denied"));
        assert_eq!(
            not_found_as_none(denied).unwrap_err().kind(),
            GcsErrorKind::Auth
        );
    }

    #[tokio::test]
    async fn move_rejects_the_same_object() {
        let client = GcsClient::new("my-bucket", user_auth()).await.unwrap();
        let err = client.move_object("a.txt", "a.txt").await.unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Config);
    }

    #[tokio::test]
    async fn signed_url_rejects_a_lifetime_over_seven_days() {
        let client = GcsClient::new(
            "my-bucket",
            Auth::User {
                client_id: "id".to_string(),
                client_secret: "secret".to_string(),
                refresh_token: "refresh".to_string(),
                signer_email: "signer@example.iam.gserviceaccount.com".to_string(),
            },
        )
        .await
        .unwrap();
        let err = client
            .signed_get_url("a", Some(Duration::from_secs(604_801)), None)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Config);
    }

    #[tokio::test]
    async fn signed_url_rejects_a_missing_or_zero_lifetime() {
        let client = GcsClient::new(
            "my-bucket",
            Auth::User {
                client_id: "id".to_string(),
                client_secret: "secret".to_string(),
                refresh_token: "refresh".to_string(),
                signer_email: "signer@example.iam.gserviceaccount.com".to_string(),
            },
        )
        .await
        .unwrap();
        for lifetime in [None, Some(Duration::ZERO), Some(Duration::from_millis(500))] {
            let err = client
                .signed_get_url("a", lifetime, None)
                .await
                .unwrap_err();
            assert_eq!(err.kind(), GcsErrorKind::Auth);
        }
    }

    #[test]
    fn path_only_omits_the_query() {
        assert_eq!(
            path_only(
                "https://storage.googleapis.com/upload/storage/v1/b/bucket/o?uploadType=resumable&upload_id=secret"
            ),
            "/upload/storage/v1/b/bucket/o"
        );
        assert_eq!(path_only("/obj?upload_id=secret"), "/obj");
    }

    #[tokio::test]
    async fn ops_log_writes_the_bucket_and_omits_the_query() {
        let client = GcsClient::new("my-bucket", user_auth()).await.unwrap();
        let mut config = ShipperConfig::new("storage");
        config.endpoint = Some("http://logs.example".to_string());
        let shipper = LogShipper::new(config).unwrap();
        client.attach_ops_log(shipper.clone());
        client.record_op(OpsRecord {
            level: LogLevel::Info,
            message: "gcs response",
            method: "GET",
            url: "https://storage.googleapis.com/storage/v1/b/my-bucket/o/file?upload_id=secret",
            status: Some(200),
            latency_ms: Some(3),
            extra: &[],
        });
        let events = shipper.buffered_events();
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
    async fn ops_log_without_a_shipper_does_not_panic() {
        let client = GcsClient::new("my-bucket", user_auth()).await.unwrap();
        client.record_op(OpsRecord {
            level: LogLevel::Warn,
            message: "gcs request failed",
            method: "GET",
            url: "https://storage.googleapis.com/storage/v1/b/my-bucket/o/file",
            status: None,
            latency_ms: None,
            extra: &[],
        });
    }

    #[test]
    fn http_status_levels_are_warn_and_error() {
        assert_eq!(level_for_status(200), None);
        assert_eq!(level_for_status(308), None);
        assert_eq!(level_for_status(404), Some(LogLevel::Warn));
        assert_eq!(level_for_status(503), Some(LogLevel::Error));
    }
}
