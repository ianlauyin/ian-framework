mod credentials;
mod object;
mod request;
mod rewrite_step;
mod upload;

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;

use crate::gcp::{http_client_builder, CachedToken};
use crate::logging::{LogLevel, LogShipper};

use crate::gcs::encode::{encode_object_id, encode_seg};
use crate::gcs::{GcsError, GcsErrorKind, API_ROOT};

use credentials::{Auth, SignedParts};
use request::{logged_error, Call, OpsRecord};
use rewrite_step::RewriteStep;
use upload::{ResumableUpload, UploadSource};

pub use object::{ListPage, ListQuery, ObjectMeta, ObjectReader};
pub use upload::{PutBody, PutOptions, PutRequest};

const TRANSFER_TIMEOUT: Duration = Duration::from_secs(180);

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

    pub async fn from_adc(
        bucket_name: impl Into<String>,
        signer_email: Option<&str>,
    ) -> Result<Self, GcsError> {
        Self::new(bucket_name, Auth::from_adc(signer_email).await?).await
    }

    pub async fn get_bytes(&self, name: &str) -> Result<Vec<u8>, GcsError> {
        self.open_object(name).await?.read_to_vec().await
    }

    pub async fn get_bytes_opt(&self, name: &str) -> Result<Option<Vec<u8>>, GcsError> {
        not_found_as_none(self.get_bytes(name).await)
    }

    pub async fn open_object(&self, name: &str) -> Result<ObjectReader, GcsError> {
        let response = self.open_response(name).await?;
        Ok(ObjectReader { response })
    }

    async fn open_response(&self, name: &str) -> Result<reqwest::Response, GcsError> {
        validate_object_name(name)?;
        self.media_response(name).await
    }

    async fn media_response(&self, name: &str) -> Result<reqwest::Response, GcsError> {
        self.send(&Call {
            method: reqwest::Method::GET,
            url: format!("{}?alt=media", object_url(self.bucket(), name)),
            ..Default::default()
        })
        .await
    }

    pub async fn open_object_opt(&self, name: &str) -> Result<Option<ObjectReader>, GcsError> {
        not_found_as_none(self.open_object(name).await)
    }

    pub async fn stat(&self, name: &str) -> Result<ObjectMeta, GcsError> {
        let response = self.stat_response(name).await?;
        ObjectMeta::from_response(response).await
    }

    async fn stat_response(&self, name: &str) -> Result<reqwest::Response, GcsError> {
        validate_object_name(name)?;
        self.object_response(name).await
    }

    async fn object_response(&self, name: &str) -> Result<reqwest::Response, GcsError> {
        self.send(&Call {
            method: reqwest::Method::GET,
            url: object_url(self.bucket(), name),
            ..Default::default()
        })
        .await
    }

    pub async fn stat_opt(&self, name: &str) -> Result<Option<ObjectMeta>, GcsError> {
        not_found_as_none(self.stat(name).await)
    }

    pub async fn list(&self, query: &ListQuery) -> Result<ListPage, GcsError> {
        let response = self.list_response(query).await?;
        list_page(response).await
    }

    async fn list_response(&self, query: &ListQuery) -> Result<reqwest::Response, GcsError> {
        self.send(&Call {
            method: reqwest::Method::GET,
            url: query.url(self.bucket()),
            ..Default::default()
        })
        .await
    }

    pub async fn put(&self, request: &PutRequest) -> Result<ObjectMeta, GcsError> {
        let content_type = checked_put(request)?;
        self.put_checked(request, content_type).await
    }

    async fn put_checked(
        &self,
        request: &PutRequest,
        content_type: String,
    ) -> Result<ObjectMeta, GcsError> {
        let mut source = UploadSource::open(&request.body).await?;
        self.upload_prepared(request, &content_type, &mut source)
            .await
    }

    async fn upload_prepared(
        &self,
        request: &PutRequest,
        content_type: &str,
        source: &mut UploadSource<'_>,
    ) -> Result<ObjectMeta, GcsError> {
        let upload = self
            .start_upload(request, content_type, source.len())
            .await?;
        upload.run(source).await
    }

    async fn start_upload<'a>(
        &'a self,
        request: &PutRequest,
        content_type: &'a str,
        total: u64,
    ) -> Result<ResumableUpload<'a>, GcsError> {
        ResumableUpload::start(
            self,
            request.name.as_str(),
            content_type,
            &request.options.metadata,
            total,
        )
        .await
    }

    pub async fn rewrite_object(&self, from: &str, to: &str) -> Result<ObjectMeta, GcsError> {
        check_rewrite_names(from, to)?;
        self.rewrite_checked(from, to).await
    }

    async fn rewrite_checked(&self, from: &str, to: &str) -> Result<ObjectMeta, GcsError> {
        let mut token = None;
        loop {
            match self.step_rewrite(from, to, token).await {
                Ok(RewriteProgress::Next(next)) => token = next,
                other => return finish_rewrite(other),
            }
        }
    }

    async fn step_rewrite(
        &self,
        from: &str,
        to: &str,
        token: Option<String>,
    ) -> Result<RewriteProgress, GcsError> {
        let bytes = self.rewrite_bytes(from, to, token.as_deref()).await?;
        step_from_bytes(&bytes, token.as_deref())
    }

    async fn rewrite_bytes(
        &self,
        from: &str,
        to: &str,
        token: Option<&str>,
    ) -> Result<Vec<u8>, GcsError> {
        let response = self.rewrite_response(from, to, token).await?;
        response_bytes(response, "rewrite response is not bytes").await
    }

    async fn rewrite_response(
        &self,
        from: &str,
        to: &str,
        token: Option<&str>,
    ) -> Result<reqwest::Response, GcsError> {
        self.send(&Call {
            method: reqwest::Method::POST,
            url: RewriteStep::url(self.bucket(), from, to, token),
            json: Some(serde_json::json!({})),
            ..Default::default()
        })
        .await
    }

    pub async fn patch_metadata(
        &self,
        name: &str,
        metadata: &BTreeMap<String, String>,
    ) -> Result<ObjectMeta, GcsError> {
        checked_patch(name, metadata)?;
        self.patch_checked(name, metadata).await
    }

    async fn patch_checked(
        &self,
        name: &str,
        metadata: &BTreeMap<String, String>,
    ) -> Result<ObjectMeta, GcsError> {
        let response = self.patch_response(name, metadata).await?;
        ObjectMeta::from_response(response).await
    }

    async fn patch_response(
        &self,
        name: &str,
        metadata: &BTreeMap<String, String>,
    ) -> Result<reqwest::Response, GcsError> {
        self.send(&Call {
            method: reqwest::Method::PATCH,
            url: object_url(self.bucket(), name),
            json: Some(serde_json::json!({ "metadata": metadata })),
            ..Default::default()
        })
        .await
    }

    pub async fn delete_object(&self, name: &str) -> Result<(), GcsError> {
        validate_object_name(name)?;
        self.delete_url(&object_url(self.bucket(), name)).await
    }

    async fn delete_url(&self, url: &str) -> Result<(), GcsError> {
        let result = self
            .send(&Call {
                method: reqwest::Method::DELETE,
                url: url.to_string(),
                ..Default::default()
            })
            .await;
        delete_result(result).await
    }

    pub async fn move_object(&self, from: &str, to: &str) -> Result<ObjectMeta, GcsError> {
        if from == to {
            return Err(GcsError::new(
                GcsErrorKind::Config,
                "move source and destination are the same object",
            ));
        }
        self.move_different(from, to).await
    }

    async fn move_different(&self, from: &str, to: &str) -> Result<ObjectMeta, GcsError> {
        let meta = self.rewrite_object(from, to).await?;
        self.delete_moved(from, to, meta).await
    }

    async fn delete_moved(
        &self,
        from: &str,
        to: &str,
        meta: ObjectMeta,
    ) -> Result<ObjectMeta, GcsError> {
        match self.delete_object(from).await {
            Ok(()) => Ok(meta),
            Err(delete_err) => self.undo_move(from, to, delete_err).await,
        }
    }

    // A failed delete does not prove that the source still exists. Delete the copy only
    // when the source is confirmed, else the copy can be the only one.
    async fn undo_move(
        &self,
        from: &str,
        to: &str,
        delete_err: GcsError,
    ) -> Result<ObjectMeta, GcsError> {
        match self.stat_opt(from).await {
            Ok(found) => self.undo_known(to, delete_err, found).await,
            Err(check_err) => Err(source_unchecked(delete_err, check_err)),
        }
    }

    async fn undo_known(
        &self,
        to: &str,
        delete_err: GcsError,
        found: Option<ObjectMeta>,
    ) -> Result<ObjectMeta, GcsError> {
        match found {
            Some(_) => self.undo_present(to, delete_err).await,
            None => Err(source_gone(delete_err)),
        }
    }

    async fn undo_present(&self, to: &str, delete_err: GcsError) -> Result<ObjectMeta, GcsError> {
        match self.delete_object(to).await {
            Ok(()) => Err(delete_err),
            Err(cleanup_err) => Err(both_names_remain(delete_err, cleanup_err)),
        }
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
        parts.sign(self).await
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

    fn record_op(&self, op: OpsRecord<'_>) {
        let Some(shipper) = lock_ops(&self.inner.ops_log).clone() else {
            return;
        };
        shipper.record(op.into_event(self.bucket()));
    }

    async fn send(&self, call: &Call) -> Result<reqwest::Response, GcsError> {
        let mut retried = false;
        loop {
            match self.send_once(call, retried).await? {
                SendStep::Retry => retried = true,
                SendStep::Done(response) => return Ok(response),
            }
        }
    }

    async fn send_once(&self, call: &Call, retried: bool) -> Result<SendStep, GcsError> {
        let token = self.access_token().await?;
        let started = Instant::now();
        let response = match self.dispatch(call, &token).await {
            Ok(response) => response,
            Err(err) => return self.transport_failed(call, started, err),
        };
        self.classify(call, response, retried, started).await
    }

    async fn dispatch(
        &self,
        call: &Call,
        token: &str,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let request = self.inner.http.request(call.method.clone(), &call.url);
        let request = request.bearer_auth(token);
        let request = apply_headers(request, &call.headers);
        let request = apply_timeout(request, call.timeout);
        apply_body(request, call).send().await
    }

    fn transport_failed(
        &self,
        call: &Call,
        started: Instant,
        err: reqwest::Error,
    ) -> Result<SendStep, GcsError> {
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
        Err(GcsError::with_source(
            GcsErrorKind::Transport,
            "gcs request failed",
            err.without_url(),
        ))
    }

    async fn classify(
        &self,
        call: &Call,
        response: reqwest::Response,
        retried: bool,
        started: Instant,
    ) -> Result<SendStep, GcsError> {
        let status = response.status().as_u16();
        let latency_ms = millis_since(started);
        if unauthorized_retry(status, retried) {
            self.reject_token(call, response, status, latency_ms).await;
            return Ok(SendStep::Retry);
        }
        self.finish_response(call, response, status, latency_ms)
            .await
    }

    async fn reject_token(
        &self,
        call: &Call,
        response: reqwest::Response,
        status: u16,
        latency_ms: u64,
    ) {
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
    }

    async fn finish_response(
        &self,
        call: &Call,
        response: reqwest::Response,
        status: u16,
        latency_ms: u64,
    ) -> Result<SendStep, GcsError> {
        self.note_status(call, status, latency_ms);
        if accepted_status(response.status(), status, call.extra_status) {
            return Ok(SendStep::Done(response));
        }
        Err(GcsError::from_response(response).await)
    }

    fn note_status(&self, call: &Call, status: u16, latency_ms: u64) {
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
    }

    async fn access_token(&self) -> Result<String, GcsError> {
        let mut guard = self.inner.token.lock().await;
        if let Some(token) = fresh_token(guard.as_ref()) {
            return Ok(token);
        }
        self.store_fetched(&mut guard).await
    }

    async fn store_fetched(&self, guard: &mut Option<CachedToken>) -> Result<String, GcsError> {
        let fetched = self.inner.auth.fetch_token(&self.inner.http).await;
        self.warn_token_error(&fetched);
        let token = fetched?;
        let value = token.value.clone();
        *guard = Some(token);
        Ok(value)
    }

    fn warn_token_error(&self, fetched: &Result<CachedToken, GcsError>) {
        if let Err(err) = fetched {
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

enum SendStep {
    Retry,
    Done(reqwest::Response),
}

enum RewriteProgress {
    Done(ObjectMeta),
    Next(Option<String>),
}

fn fresh_token(cached: Option<&CachedToken>) -> Option<String> {
    let cached = cached?;
    if Instant::now() < cached.expires_at {
        Some(cached.value.clone())
    } else {
        None
    }
}

fn unauthorized_retry(status: u16, retried: bool) -> bool {
    status == 401 && !retried
}

fn accepted_status(status: reqwest::StatusCode, code: u16, extra: &[u16]) -> bool {
    status.is_success() || extra.contains(&code)
}

fn apply_headers(
    mut request: reqwest::RequestBuilder,
    headers: &[(reqwest::header::HeaderName, String)],
) -> reqwest::RequestBuilder {
    for (name, value) in headers {
        request = request.header(name, value);
    }
    request
}

fn apply_timeout(
    request: reqwest::RequestBuilder,
    timeout: Option<Duration>,
) -> reqwest::RequestBuilder {
    if let Some(timeout) = timeout {
        request.timeout(timeout)
    } else {
        request
    }
}

fn apply_body(request: reqwest::RequestBuilder, call: &Call) -> reqwest::RequestBuilder {
    if let Some(json) = &call.json {
        request.json(json)
    } else {
        apply_bytes(request, call.bytes.as_ref())
    }
}

fn apply_bytes(
    request: reqwest::RequestBuilder,
    bytes: Option<&Vec<u8>>,
) -> reqwest::RequestBuilder {
    if let Some(bytes) = bytes {
        request.body(bytes.clone())
    } else {
        request
    }
}

async fn list_page(response: reqwest::Response) -> Result<ListPage, GcsError> {
    let bytes = response_bytes(response, "list response is not bytes").await?;
    ListPage::from_slice(&bytes)
}

async fn response_bytes(
    response: reqwest::Response,
    message: &'static str,
) -> Result<Vec<u8>, GcsError> {
    match response.bytes().await {
        Ok(bytes) => Ok(bytes.to_vec()),
        Err(err) => Err(GcsError::with_source(
            GcsErrorKind::Transport,
            message,
            err.without_url(),
        )),
    }
}

fn checked_put(request: &PutRequest) -> Result<String, GcsError> {
    validate_object_name(request.name.as_str())?;
    put_content_type(request)
}

fn put_content_type(request: &PutRequest) -> Result<String, GcsError> {
    let content_type = request.options.content_type()?;
    accept_metadata(&request.options.metadata, content_type)
}

fn accept_metadata(
    metadata: &BTreeMap<String, String>,
    content_type: String,
) -> Result<String, GcsError> {
    validate_metadata(metadata)?;
    Ok(content_type)
}

fn check_rewrite_names(from: &str, to: &str) -> Result<(), GcsError> {
    validate_object_name(from)?;
    validate_object_name(to)
}

fn step_from_bytes(bytes: &[u8], token: Option<&str>) -> Result<RewriteProgress, GcsError> {
    let parsed = RewriteStep::from_slice(bytes)?;
    step_from_parsed(token, parsed)
}

fn step_from_parsed(token: Option<&str>, parsed: RewriteStep) -> Result<RewriteProgress, GcsError> {
    match parsed {
        RewriteStep::Done(meta) => Ok(RewriteProgress::Done(meta)),
        RewriteStep::Next(next) => changed_token(token, next),
    }
}

fn changed_token(previous: Option<&str>, next: String) -> Result<RewriteProgress, GcsError> {
    if previous == Some(next.as_str()) {
        Err(GcsError::new(
            GcsErrorKind::Api,
            "rewrite token did not change",
        ))
    } else {
        Ok(RewriteProgress::Next(Some(next)))
    }
}

fn finish_rewrite(step: Result<RewriteProgress, GcsError>) -> Result<ObjectMeta, GcsError> {
    match step {
        Ok(RewriteProgress::Done(meta)) => Ok(meta),
        other => rewrite_failed(other),
    }
}

fn rewrite_failed(step: Result<RewriteProgress, GcsError>) -> Result<ObjectMeta, GcsError> {
    match step {
        Err(err) => Err(err),
        Ok(_) => Err(GcsError::new(
            GcsErrorKind::Api,
            "rewrite token did not change",
        )),
    }
}

fn checked_patch(name: &str, metadata: &BTreeMap<String, String>) -> Result<(), GcsError> {
    validate_object_name(name)?;
    validate_metadata(metadata)
}

async fn delete_result(result: Result<reqwest::Response, GcsError>) -> Result<(), GcsError> {
    match result {
        Ok(response) => finish_delete(response).await,
        Err(err) => missing_or_err(err),
    }
}

async fn finish_delete(response: reqwest::Response) -> Result<(), GcsError> {
    let _ = response.bytes().await;
    Ok(())
}

fn missing_or_err(err: GcsError) -> Result<(), GcsError> {
    if err.kind() == GcsErrorKind::NotFound {
        Ok(())
    } else {
        Err(err)
    }
}

fn both_names_remain(delete_err: GcsError, cleanup_err: GcsError) -> GcsError {
    GcsError::with_source(
        delete_err.kind(),
        format!(
            "move could not delete the source ({delete_err}) and could not delete \
             the destination ({cleanup_err}); the object exists at both names"
        ),
        delete_err,
    )
}

fn source_gone(delete_err: GcsError) -> GcsError {
    GcsError::with_source(
        delete_err.kind(),
        format!(
            "move could not delete the source ({delete_err}) but the source is gone; \
             the object exists only at the destination"
        ),
        delete_err,
    )
}

fn source_unchecked(delete_err: GcsError, check_err: GcsError) -> GcsError {
    GcsError::with_source(
        delete_err.kind(),
        format!(
            "move could not delete the source ({delete_err}) and could not check \
             the source ({check_err}); the object may exist at both names"
        ),
        delete_err,
    )
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
    let builder = http_client_builder();
    #[cfg(target_os = "linux")]
    let builder = builder.tcp_user_timeout(TRANSFER_TIMEOUT);
    builder.build().map_err(|err| {
        GcsError::with_source(GcsErrorKind::Transport, "cannot build the HTTP client", err)
    })
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
    use crate::gcp::UserCredentials;
    use crate::logging::ShipperConfig;

    use super::*;
    use crate::logging::tests::buffered_events;

    #[test]
    fn metadata_rejects_an_empty_key_or_a_line_break() {
        let mut metadata = BTreeMap::new();
        assert!(validate_metadata(&metadata).is_ok());
        metadata.insert("owner".to_string(), "app".to_string());
        assert!(validate_metadata(&metadata).is_ok());
        metadata.insert(String::new(), "app".to_string());
        assert_eq!(
            validate_metadata(&metadata).unwrap_err().kind(),
            GcsErrorKind::Config
        );
        metadata.clear();
        metadata.insert("owner".to_string(), "a\nb".to_string());
        assert!(validate_metadata(&metadata).is_err());
        metadata.clear();
        metadata.insert("a\nb".to_string(), "app".to_string());
        assert!(validate_metadata(&metadata).is_err());
    }

    #[test]
    fn object_url_encodes_slashes() {
        assert_eq!(
            object_url("buck", "a/c"),
            "https://storage.googleapis.com/storage/v1/b/buck/o/a%2Fc"
        );
    }

    fn user_auth() -> Auth {
        Auth::User {
            credentials: UserCredentials {
                client_id: "client".to_string(),
                client_secret: "secret".to_string(),
                refresh_token: "refresh".to_string(),
                quota_project_id: None,
            },
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
            credentials: UserCredentials {
                client_id: String::new(),
                client_secret: "secret".to_string(),
                refresh_token: "refresh".to_string(),
                quota_project_id: None,
            },
            signer_email: "signer@example.iam.gserviceaccount.com".to_string(),
        };
        let err = GcsClient::new("my-bucket", auth).await.unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Config);
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
        let client = GcsClient::new("my-bucket", user_auth()).await.unwrap();
        let err = client
            .signed_get_url("a", Some(Duration::from_secs(604_801)), None)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Config);
    }

    #[tokio::test]
    async fn signed_url_rejects_a_missing_or_zero_lifetime() {
        let client = GcsClient::new("my-bucket", user_auth()).await.unwrap();
        for lifetime in [None, Some(Duration::ZERO), Some(Duration::from_millis(500))] {
            let err = client
                .signed_get_url("a", lifetime, None)
                .await
                .unwrap_err();
            assert_eq!(err.kind(), GcsErrorKind::Auth);
        }
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
        let events = buffered_events(&shipper);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "GCS/GET:/storage/v1/b/my-bucket/o/file");
        assert_eq!(events[0].id, "");
        assert!(events[0].context.is_empty());
        assert_eq!(events[0].info["status"], "200");
        assert_eq!(events[0].info["message"], "gcs response");
        assert_eq!(events[0].latency_ms, Some(3));
        assert_eq!(
            events[0]
                .info
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

    #[tokio::test]
    async fn send_error_source_omits_the_url() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let client = GcsClient::new("my-bucket", user_auth()).await.unwrap();
        *client.inner.token.lock().await = Some(CachedToken {
            value: "token".to_string(),
            expires_at: Instant::now() + Duration::from_secs(600),
        });
        let err = client
            .send(&Call {
                method: reqwest::Method::PUT,
                url: format!("http://{addr}/upload?upload_id=secret"),
                ..Default::default()
            })
            .await
            .unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Transport);
        let source = std::error::Error::source(&err).unwrap().to_string();
        assert!(!source.contains("secret"), "{source}");
        assert!(!source.contains("upload_id"), "{source}");
    }

    #[test]
    fn http_status_levels_are_warn_and_error() {
        assert_eq!(level_for_status(200), None);
        assert_eq!(level_for_status(308), None);
        assert_eq!(level_for_status(404), Some(LogLevel::Warn));
        assert_eq!(level_for_status(503), Some(LogLevel::Error));
    }
}
