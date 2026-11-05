pub(crate) mod credentials;
pub(crate) mod object;
pub(crate) mod request;
pub(crate) mod rewrite_step;
pub(crate) mod upload;

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;

use crate::gcp::{http_client_builder, CachedToken, METADATA_TOKEN_URL};
use crate::logging::{LogLevel, Logger};

use crate::gcs::encode::{encode_object_id, encode_seg};
use crate::gcs::{
    gcs_response_exception, is_retryable_status, API_ROOT, IAM_ROOT, UPLOAD_HOST_ROOT, UPLOAD_ROOT,
};

use crate::exception;
use crate::exception::Exception;
use credentials::{Auth, SignedParts};
use request::{logged_error, Call, OpsRecord};
use rewrite_step::RewriteStep;
use upload::{ResumableUpload, UploadSource};

pub use object::{ListPage, ListQuery, ObjectMeta, ObjectReader};
pub use upload::{PutBody, PutOptions, PutRequest};

pub(crate) const TRANSFER_TIMEOUT: Duration = Duration::from_secs(180);
pub(crate) const SEND_ATTEMPTS: u32 = 4;
const SEND_BACKOFF: Duration = Duration::from_millis(250);
// The most rewrite calls for one object. A rewrite that needs more is a fault.
const MAX_REWRITE_STEPS: u32 = 10_000;
// GCS refuses a longer object name.
pub(crate) const MAX_OBJECT_NAME_BYTES: usize = 1024;

#[derive(Clone)]
pub struct GcsClient {
    pub(crate) inner: Arc<Inner>,
}

impl GcsClient {
    pub fn bucket(&self) -> &str {
        &self.inner.bucket
    }

    pub fn signer_email(&self) -> &str {
        &self.inner.email
    }

    pub async fn from_adc(
        bucket_name: impl Into<String>,
        signer_email: Option<&str>,
    ) -> Result<Self, Exception> {
        Self::new(bucket_name, Auth::from_adc(signer_email).await?).await
    }

    pub async fn get_bytes(&self, name: &str) -> Result<Vec<u8>, Exception> {
        self.open_object(name).await?.read_to_vec().await
    }

    pub async fn get_bytes_opt(&self, name: &str) -> Result<Option<Vec<u8>>, Exception> {
        not_found_as_none(self.get_bytes(name).await)
    }

    pub async fn open_object(&self, name: &str) -> Result<ObjectReader, Exception> {
        validate_object_name(name)?;
        let response = self
            .send(&Call {
                method: reqwest::Method::GET,
                url: format!("{}?alt=media", self.object_url(name)),
                ..Default::default()
            })
            .await?;
        Ok(ObjectReader { response })
    }

    pub async fn open_object_opt(&self, name: &str) -> Result<Option<ObjectReader>, Exception> {
        not_found_as_none(self.open_object(name).await)
    }

    pub async fn stat(&self, name: &str) -> Result<ObjectMeta, Exception> {
        validate_object_name(name)?;
        let response = self
            .send(&Call {
                method: reqwest::Method::GET,
                url: self.object_url(name),
                ..Default::default()
            })
            .await?;
        ObjectMeta::from_response(response).await
    }

    pub async fn stat_opt(&self, name: &str) -> Result<Option<ObjectMeta>, Exception> {
        not_found_as_none(self.stat(name).await)
    }

    pub async fn list(&self, query: &ListQuery) -> Result<ListPage, Exception> {
        let response = self
            .send(&Call {
                method: reqwest::Method::GET,
                url: query.url(&self.inner.endpoints.api_root, self.bucket()),
                ..Default::default()
            })
            .await?;
        let bytes = response.bytes().await.map_err(|err| {
            exception!(
                code = "GCS_TRANSPORT",
                "list response is not bytes",
                source = err.without_url(),
            )
        })?;
        ListPage::from_slice(&bytes)
    }

    pub async fn put(&self, request: &PutRequest) -> Result<ObjectMeta, Exception> {
        let name = request.name.as_str();
        let options = &request.options;
        validate_object_name(name)?;
        let content_type = options.content_type()?;
        validate_metadata(&options.metadata)?;
        let mut source = UploadSource::open(&request.body).await?;
        let upload =
            ResumableUpload::start(self, name, &content_type, &options.metadata, source.len())
                .await?;
        upload.run(&mut source).await
    }

    pub async fn rewrite_object(&self, from: &str, to: &str) -> Result<ObjectMeta, Exception> {
        validate_object_name(from)?;
        validate_object_name(to)?;
        let mut token: Option<String> = None;
        for _ in 0..MAX_REWRITE_STEPS {
            let next = match self.rewrite_step(from, to, token.as_deref()).await? {
                RewriteStep::Done(meta) => return Ok(meta),
                RewriteStep::Next(next) => next,
            };
            if token.as_deref() == Some(next.as_str()) {
                return Err(exception!(code = "GCS_API", "rewrite token did not change"));
            }
            token = Some(next);
        }
        Err(exception!(
            code = "GCS_API",
            "rewrite did not finish within the step limit",
        ))
    }

    pub async fn patch_metadata(
        &self,
        name: &str,
        metadata: &BTreeMap<String, String>,
    ) -> Result<ObjectMeta, Exception> {
        validate_object_name(name)?;
        validate_metadata(metadata)?;
        let response = self
            .send(&Call {
                method: reqwest::Method::PATCH,
                url: self.object_url(name),
                json: Some(serde_json::json!({ "metadata": metadata })),
                ..Default::default()
            })
            .await?;
        ObjectMeta::from_response(response).await
    }

    pub async fn delete_object(&self, name: &str) -> Result<(), Exception> {
        validate_object_name(name)?;
        let result = self
            .send(&Call {
                method: reqwest::Method::DELETE,
                url: self.object_url(name),
                ..Default::default()
            })
            .await;
        let response = match result {
            Ok(response) => response,
            Err(err) if err.code() == "GCS_NOT_FOUND" => return Ok(()),
            Err(err) => return Err(err),
        };
        let _ = response.bytes().await;
        Ok(())
    }

    pub async fn move_object(&self, from: &str, to: &str) -> Result<ObjectMeta, Exception> {
        if from == to {
            return Err(exception!(
                code = "GCS_CONFIG",
                "move source and destination are the same object",
            ));
        }
        let meta = self.rewrite_object(from, to).await?;
        match self.delete_object(from).await {
            Ok(()) => Ok(meta),
            Err(delete_err) => Err(self.failed_move_error(from, to, delete_err).await),
        }
    }

    pub async fn signed_get_url(
        &self,
        name: &str,
        expires_in: Option<Duration>,
        attachment_filename: Option<&str>,
    ) -> Result<String, Exception> {
        validate_object_name(name)?;
        let parts = SignedParts::v4_get(
            self.bucket(),
            name,
            self.signer_email(),
            Utc::now(),
            expires_in,
            attachment_filename,
        )?;
        parts.sign(self).await
    }

    pub fn attach_ops_log(&self, logger: Logger) {
        *lock_ops(&self.inner.ops_log) = Some(logger);
    }

    fn object_url(&self, name: &str) -> String {
        object_url(&self.inner.endpoints.api_root, self.bucket(), name)
    }

    async fn rewrite_step(
        &self,
        from: &str,
        to: &str,
        token: Option<&str>,
    ) -> Result<RewriteStep, Exception> {
        let response = self
            .send(&Call {
                method: reqwest::Method::POST,
                url: RewriteStep::url(
                    &self.inner.endpoints.api_root,
                    self.bucket(),
                    from,
                    to,
                    token,
                ),
                json: Some(serde_json::json!({})),
                ..Default::default()
            })
            .await?;
        let bytes = response.bytes().await.map_err(|err| {
            exception!(
                code = "GCS_TRANSPORT",
                "rewrite response is not bytes",
                source = err.without_url(),
            )
        })?;
        RewriteStep::from_slice(&bytes)
    }

    // A failed delete does not prove that the source still exists. Delete the copy only
    // when the source is confirmed, else the copy can be the only one.
    async fn failed_move_error(&self, from: &str, to: &str, delete_err: Exception) -> Exception {
        let kind = delete_err.code();
        let message = match self.stat_opt(from).await {
            Ok(Some(_)) => match self.delete_object(to).await {
                Ok(()) => return delete_err,
                Err(cleanup_err) => format!(
                    "move could not delete the source ({delete_err}) and could not delete \
                     the destination ({cleanup_err}); the object exists at both names"
                ),
            },
            Ok(None) => format!(
                "move could not delete the source ({delete_err}) but the source is gone; \
                 the object exists only at the destination"
            ),
            Err(check_err) => format!(
                "move could not delete the source ({delete_err}) and could not check \
                 the source ({check_err}); the object may exist at both names"
            ),
        };
        exception!(code = kind, message, source = delete_err)
    }

    pub(crate) async fn new(bucket_name: impl Into<String>, auth: Auth) -> Result<Self, Exception> {
        let bucket_name = bucket_name.into();
        validate_bucket(&bucket_name)?;
        auth.validate()?;
        let http = http_client()?;
        let email = auth.signer_email(&http).await?;
        Ok(Self {
            inner: Arc::new(Inner {
                bucket: bucket_name,
                endpoints: Endpoints::google(),
                email,
                auth,
                http,
                token: tokio::sync::Mutex::new(None),
                ops_log: std::sync::Mutex::new(None),
            }),
        })
    }

    pub(crate) fn record_op(&self, op: OpsRecord<'_>) {
        let Some(logger) = lock_ops(&self.inner.ops_log).clone() else {
            return;
        };
        logger.record(op.into_event(self.bucket()));
    }

    pub(crate) async fn send(&self, call: &Call) -> Result<reqwest::Response, Exception> {
        let mut refreshed = false;
        let mut attempts = 0u32;
        loop {
            match self.try_send(call, attempts, refreshed).await? {
                Outcome::Done(response) => return Ok(response),
                Outcome::Retry => {
                    attempts += 1;
                    tokio::time::sleep(SEND_BACKOFF * attempts).await;
                }
                Outcome::Refresh => refreshed = true,
            }
        }
    }

    async fn try_send(
        &self,
        call: &Call,
        attempts: u32,
        refreshed: bool,
    ) -> Result<Outcome, Exception> {
        let token = self.access_token().await?;
        let started = Instant::now();
        let response = match build_request(&self.inner.http, call, &token).send().await {
            Ok(response) => response,
            Err(err) => return self.failed_send(call, attempts, started, err),
        };
        let status = response.status().as_u16();
        let latency_ms = millis_since(started);
        if status == 401 && !refreshed {
            let _ = response.bytes().await;
            self.invalidate_token().await;
            self.record_status(
                call,
                "gcs access token rejected",
                LogLevel::Warn,
                status,
                latency_ms,
            );
            return Ok(Outcome::Refresh);
        }
        if let Some(level) = level_for_status(status) {
            self.record_status(call, "gcs response", level, status, latency_ms);
        }
        if response.status().is_success() || call.extra_status.contains(&status) {
            return Ok(Outcome::Done(response));
        }
        if is_retryable_status(status) && may_retry(call, attempts) {
            let _ = response.bytes().await;
            return Ok(Outcome::Retry);
        }
        Err(gcs_response_exception(response).await)
    }

    fn failed_send(
        &self,
        call: &Call,
        attempts: u32,
        started: Instant,
        err: reqwest::Error,
    ) -> Result<Outcome, Exception> {
        let error_text = logged_error(&err);
        self.record_op(OpsRecord {
            level: LogLevel::Warn,
            message: "gcs request failed",
            method: call.method.as_str(),
            url: &call.url,
            status: None,
            latency_ms: millis_since(started),
            extra: &[("error", &error_text)],
        });
        if may_retry(call, attempts) {
            return Ok(Outcome::Retry);
        }
        Err(exception!(
            code = "GCS_TRANSPORT",
            "gcs request failed",
            source = err.without_url(),
        ))
    }

    fn record_status(
        &self,
        call: &Call,
        message: &str,
        level: LogLevel,
        status: u16,
        latency_ms: u64,
    ) {
        self.record_op(OpsRecord {
            level,
            message,
            method: call.method.as_str(),
            url: &call.url,
            status: Some(status),
            latency_ms,
            extra: &[],
        });
    }

    async fn access_token(&self) -> Result<String, Exception> {
        let mut guard = self.inner.token.lock().await;
        if let Some(cached) = guard.as_ref() {
            if Instant::now() < cached.expires_at {
                return Ok(cached.value.clone());
            }
        }
        let fetched = self
            .inner
            .auth
            .fetch_token(&self.inner.http, &self.inner.endpoints.metadata_token_url)
            .await;
        if let Err(err) = &fetched {
            let error_text = logged_error(&String::from(err));
            self.record_op(OpsRecord {
                level: LogLevel::Warn,
                message: "access token request failed",
                method: "",
                url: "",
                status: None,
                latency_ms: 0,
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

pub(crate) struct Endpoints {
    pub(crate) api_root: String,
    pub(crate) upload_root: String,
    pub(crate) upload_location_prefix: String,
    pub(crate) iam_root: String,
    pub(crate) metadata_token_url: String,
}

impl Endpoints {
    pub(crate) fn google() -> Self {
        Self {
            api_root: API_ROOT.to_string(),
            upload_root: UPLOAD_ROOT.to_string(),
            upload_location_prefix: UPLOAD_HOST_ROOT.to_string(),
            iam_root: IAM_ROOT.to_string(),
            metadata_token_url: METADATA_TOKEN_URL.to_string(),
        }
    }
}

pub(crate) struct Inner {
    pub(crate) bucket: String,
    pub(crate) endpoints: Endpoints,
    pub(crate) email: String,
    pub(crate) auth: Auth,
    pub(crate) http: reqwest::Client,
    pub(crate) token: tokio::sync::Mutex<Option<CachedToken>>,
    pub(crate) ops_log: std::sync::Mutex<Option<Logger>>,
}

enum Outcome {
    Done(reqwest::Response),
    Retry,
    Refresh,
}

fn build_request(http: &reqwest::Client, call: &Call, token: &str) -> reqwest::RequestBuilder {
    let mut request = http
        .request(call.method.clone(), &call.url)
        .bearer_auth(token);
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
    request
}

pub(crate) fn not_found_as_none<T>(result: Result<T, Exception>) -> Result<Option<T>, Exception> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(err) if err.code() == "GCS_NOT_FOUND" => Ok(None),
        Err(err) => Err(err),
    }
}

fn lock_ops(slot: &std::sync::Mutex<Option<Logger>>) -> std::sync::MutexGuard<'_, Option<Logger>> {
    slot.lock().unwrap_or_else(|err| err.into_inner())
}

pub(crate) fn may_retry(call: &Call, attempts: u32) -> bool {
    !call.no_retry && attempts + 1 < SEND_ATTEMPTS
}

pub(crate) fn validate_object_name(name: &str) -> Result<(), Exception> {
    if name.is_empty()
        || name.len() > MAX_OBJECT_NAME_BYTES
        || name == "."
        || name == ".."
        || name.contains(['\r', '\n'])
    {
        return Err(exception!(
            code = "GCS_CONFIG",
            "object name is empty, too long, `.`, `..` or contains a line break",
        ));
    }
    Ok(())
}

pub(crate) fn validate_metadata(metadata: &BTreeMap<String, String>) -> Result<(), Exception> {
    for (key, value) in metadata {
        if key.is_empty() || key.contains(['\r', '\n']) || value.contains(['\r', '\n']) {
            return Err(exception!(
                code = "GCS_CONFIG",
                "metadata key is empty or metadata contains a line break",
            ));
        }
    }
    Ok(())
}

pub(crate) fn object_url(api_root: &str, bucket_name: &str, name: &str) -> String {
    format!(
        "{api_root}/b/{}/o/{}",
        encode_seg(bucket_name),
        encode_object_id(name),
    )
}

fn validate_bucket(bucket_name: &str) -> Result<(), Exception> {
    if bucket_name.is_empty()
        || bucket_name.contains(['/', '\\', ' ', '\r', '\n'])
        || !bucket_name.is_ascii()
    {
        return Err(exception!(
            code = "GCS_CONFIG",
            "bucket name is empty or contains a forbidden character",
        ));
    }
    Ok(())
}

pub(crate) fn http_client() -> Result<reqwest::Client, Exception> {
    // A stalled read ends the call on every platform, and a long download is not cut.
    let builder = http_client_builder().read_timeout(TRANSFER_TIMEOUT);
    #[cfg(target_os = "linux")]
    let builder = builder.tcp_user_timeout(TRANSFER_TIMEOUT);
    builder.build().map_err(|err| {
        exception!(
            code = "GCS_LOCAL",
            "cannot build the HTTP client",
            source = err,
        )
    })
}

fn millis_since(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

pub(crate) fn level_for_status(status: u16) -> Option<LogLevel> {
    if status >= 500 {
        Some(LogLevel::Error)
    } else if status >= 400 {
        Some(LogLevel::Warn)
    } else {
        None
    }
}
