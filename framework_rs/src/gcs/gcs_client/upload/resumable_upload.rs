use std::collections::BTreeMap;
use std::time::Duration;

use crate::logging::LogLevel;

use crate::gcs::encode::encode_seg;
use crate::gcs::gcs_client::request::{logged_error, Call, OpsRecord};
use crate::gcs::gcs_client::upload::upload_source::UploadSource;
use crate::gcs::gcs_client::{GcsClient, ObjectMeta, TRANSFER_TIMEOUT};
use crate::gcs::{GcsError, GcsErrorKind};

const UPLOAD_ROOT: &str = "https://storage.googleapis.com/upload/storage/v1";
const CHUNK_SIZE: usize = 8 * 1024 * 1024;
const CHUNK_ALIGN: u64 = 256 * 1024;
const CHUNK_ATTEMPTS: u32 = 5;
const CHUNK_BACKOFF: Duration = Duration::from_millis(200);

pub(crate) struct ResumableUpload<'a> {
    client: &'a GcsClient,
    location: String,
    content_type: &'a str,
    total: u64,
}

impl<'a> ResumableUpload<'a> {
    pub(crate) async fn start(
        client: &'a GcsClient,
        name: &str,
        content_type: &'a str,
        metadata: &BTreeMap<String, String>,
        total: u64,
    ) -> Result<Self, GcsError> {
        let response = client
            .send(&start_call(client, name, content_type, metadata, total))
            .await?;
        Self::from_start(client, content_type, total, response).await
    }

    async fn from_start(
        client: &'a GcsClient,
        content_type: &'a str,
        total: u64,
        response: reqwest::Response,
    ) -> Result<Self, GcsError> {
        let location = start_location(&response)?;
        let _ = response.bytes().await;
        Self::at_location(client, content_type, total, location)
    }

    fn at_location(
        client: &'a GcsClient,
        content_type: &'a str,
        total: u64,
        location: String,
    ) -> Result<Self, GcsError> {
        let location = https_location(location)?;
        Ok(Self {
            client,
            location,
            content_type,
            total,
        })
    }

    pub(crate) async fn run(&self, source: &mut UploadSource<'_>) -> Result<ObjectMeta, GcsError> {
        if self.total == 0 {
            return self.empty_upload().await;
        }
        self.upload_parts(source).await
    }

    async fn empty_upload(&self) -> Result<ObjectMeta, GcsError> {
        self.put_chunk_retrying(b"", 0, true).await?.expect_done()
    }

    async fn upload_parts(&self, source: &mut UploadSource<'_>) -> Result<ObjectMeta, GcsError> {
        let mut offset = 0u64;
        let mut rewinds = 0u32;
        loop {
            return match self.next_part(source, &mut offset, &mut rewinds).await {
                Ok(Part::Continue) => continue,
                other => finish_part(other),
            };
        }
    }

    async fn next_part(
        &self,
        source: &mut UploadSource<'_>,
        offset: &mut u64,
        rewinds: &mut u32,
    ) -> Result<Part, GcsError> {
        if *offset >= self.total {
            return Ok(Part::Finished);
        }
        self.read_and_send(source, offset, rewinds).await
    }

    async fn read_and_send(
        &self,
        source: &mut UploadSource<'_>,
        offset: &mut u64,
        rewinds: &mut u32,
    ) -> Result<Part, GcsError> {
        let chunk = self.read_part(source, *offset).await?;
        self.apply_part(offset, rewinds, chunk).await
    }

    async fn read_part(
        &self,
        source: &mut UploadSource<'_>,
        offset: u64,
    ) -> Result<PartChunk, GcsError> {
        let spec = part_spec(self.total, offset);
        let chunk = source.read_chunk(offset, spec.len).await?;
        Ok(PartChunk {
            chunk: chunk.into_owned(),
            is_last: spec.is_last,
        })
    }

    async fn apply_part(
        &self,
        offset: &mut u64,
        rewinds: &mut u32,
        chunk: PartChunk,
    ) -> Result<Part, GcsError> {
        let step = self
            .put_chunk_retrying(&chunk.chunk, *offset, chunk.is_last)
            .await?;
        self.after_part(offset, rewinds, step)
    }

    fn after_part(
        &self,
        offset: &mut u64,
        rewinds: &mut u32,
        step: UploadStep,
    ) -> Result<Part, GcsError> {
        match step {
            UploadStep::Done(meta) => Ok(Part::Done(meta)),
            UploadStep::Continue { next } => self.advance_part(offset, rewinds, next),
        }
    }

    fn advance_part(
        &self,
        offset: &mut u64,
        rewinds: &mut u32,
        next: u64,
    ) -> Result<Part, GcsError> {
        advance_offset(offset, rewinds, next, self.total)?;
        Ok(Part::Continue)
    }

    async fn put_chunk_retrying(
        &self,
        chunk: &[u8],
        start: u64,
        finished: bool,
    ) -> Result<UploadStep, GcsError> {
        let mut offset = start;
        let mut attempt = 0u32;
        loop {
            match self
                .one_attempt(chunk, start, finished, offset, attempt)
                .await
            {
                Attempt::Again {
                    offset: next,
                    attempt: next_attempt,
                } => {
                    offset = next;
                    attempt = next_attempt;
                }
                Attempt::Stop(result) => return result,
            }
        }
    }

    async fn one_attempt(
        &self,
        chunk: &[u8],
        start: u64,
        finished: bool,
        offset: u64,
        attempt: u32,
    ) -> Attempt {
        let chunk_len = chunk.len() as u64;
        if chunk_suffix_len(chunk_len, start, offset, finished).is_none() {
            return Attempt::Stop(Ok(UploadStep::Continue { next: offset }));
        }
        self.retry_attempt(chunk, start, offset, attempt).await
    }

    async fn retry_attempt(&self, chunk: &[u8], start: u64, offset: u64, attempt: u32) -> Attempt {
        let attempt = attempt + 1;
        if attempt > CHUNK_ATTEMPTS {
            return Attempt::Stop(Err(GcsError::new(
                GcsErrorKind::Transport,
                "resumable chunk failed after retries",
            )));
        }
        self.send_attempt(chunk, start, offset, attempt).await
    }

    async fn send_attempt(&self, chunk: &[u8], start: u64, offset: u64, attempt: u32) -> Attempt {
        if attempt > 1 {
            tokio::time::sleep(retry_delay(attempt - 1)).await;
        }
        self.put_then_probe(chunk, start, offset, attempt).await
    }

    async fn put_then_probe(&self, chunk: &[u8], start: u64, offset: u64, attempt: u32) -> Attempt {
        let chunk_len = chunk.len() as u64;
        let piece = &chunk[(offset - start) as usize..];
        match self
            .after_upload(piece, start, chunk_len, offset, attempt)
            .await
        {
            PutFollow::Probe => {
                self.after_probe_attempt(start, chunk_len, offset, attempt)
                    .await
            }
            other => follow_put(other),
        }
    }

    async fn after_upload(
        &self,
        piece: &[u8],
        start: u64,
        chunk_len: u64,
        offset: u64,
        attempt: u32,
    ) -> PutFollow {
        let sent = self.upload_chunk_once(piece, offset).await;
        self.follow_upload(sent, start, chunk_len, offset, attempt)
            .await
    }

    async fn follow_upload(
        &self,
        sent: Result<UploadStep, GcsError>,
        start: u64,
        chunk_len: u64,
        offset: u64,
        attempt: u32,
    ) -> PutFollow {
        match sent {
            Ok(step) => follow_upload_ok(step, start, chunk_len, offset),
            Err(err) => self.follow_upload_err(err, attempt, offset).await,
        }
    }

    async fn follow_upload_err(&self, err: GcsError, attempt: u32, offset: u64) -> PutFollow {
        if err.is_retryable() {
            self.warn_resumable("gcs resumable chunk failed", attempt, offset, &err);
            return PutFollow::Probe;
        }
        PutFollow::Stop(Err(err))
    }

    async fn after_probe_attempt(
        &self,
        start: u64,
        chunk_len: u64,
        offset: u64,
        attempt: u32,
    ) -> Attempt {
        let probed = self.probe_session().await;
        self.follow_probe(probed, start, chunk_len, offset, attempt)
            .await
    }

    async fn follow_probe(
        &self,
        probed: Result<UploadStep, GcsError>,
        start: u64,
        chunk_len: u64,
        offset: u64,
        attempt: u32,
    ) -> Attempt {
        match probed {
            Ok(step) => follow_probe_ok(step, start, chunk_len, offset, attempt),
            Err(err) => self.follow_probe_err(err, attempt, offset).await,
        }
    }

    async fn follow_probe_err(&self, err: GcsError, attempt: u32, offset: u64) -> Attempt {
        if err.is_retryable() {
            self.warn_resumable("gcs resumable probe failed", attempt, offset, &err);
            return Attempt::Again { offset, attempt };
        }
        Attempt::Stop(Err(err))
    }

    fn warn_resumable(&self, message: &str, attempt: u32, offset: u64, err: &GcsError) {
        let attempt_text = attempt.to_string();
        let offset_text = offset.to_string();
        let error_text = logged_error(err);
        self.client.record_op(OpsRecord {
            level: LogLevel::Warn,
            message,
            method: "PUT",
            url: &self.location,
            status: err.status(),
            latency_ms: None,
            extra: &[
                ("attempt", &attempt_text),
                ("offset", &offset_text),
                ("error", &error_text),
            ],
        });
    }

    async fn upload_chunk_once(&self, chunk: &[u8], offset: u64) -> Result<UploadStep, GcsError> {
        let range = chunk_content_range(offset, chunk.len() as u64, self.total);
        let response = self
            .client
            .send(&Call {
                method: reqwest::Method::PUT,
                url: self.location.clone(),
                headers: vec![
                    (reqwest::header::CONTENT_TYPE, self.content_type.to_string()),
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

    async fn probe_session(&self) -> Result<UploadStep, GcsError> {
        let response = self
            .client
            .send(&Call {
                method: reqwest::Method::PUT,
                url: self.location.clone(),
                headers: vec![
                    (
                        reqwest::header::CONTENT_RANGE,
                        probe_content_range(self.total),
                    ),
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
}

enum Part {
    Done(ObjectMeta),
    Continue,
    Finished,
}

struct PartSpec {
    len: usize,
    is_last: bool,
}

struct PartChunk {
    chunk: Vec<u8>,
    is_last: bool,
}

enum Attempt {
    Again { offset: u64, attempt: u32 },
    Stop(Result<UploadStep, GcsError>),
}

enum PutFollow {
    Stop(Result<UploadStep, GcsError>),
    Again { offset: u64, attempt: u32 },
    Probe,
}

fn start_call(
    client: &GcsClient,
    name: &str,
    content_type: &str,
    metadata: &BTreeMap<String, String>,
    total: u64,
) -> Call {
    let url = format!(
        "{UPLOAD_ROOT}/b/{}/o?uploadType=resumable",
        encode_seg(client.bucket()),
    );
    let mut body = serde_json::json!({
        "name": name,
        "contentType": content_type,
    });
    if !metadata.is_empty() {
        body["metadata"] = serde_json::json!(metadata);
    }
    Call {
        method: reqwest::Method::POST,
        url,
        headers: start_headers(content_type, total),
        json: Some(body),
        ..Default::default()
    }
}

fn start_headers(content_type: &str, total: u64) -> Vec<(reqwest::header::HeaderName, String)> {
    vec![
        (
            reqwest::header::HeaderName::from_static("x-upload-content-type"),
            content_type.to_string(),
        ),
        (
            reqwest::header::HeaderName::from_static("x-upload-content-length"),
            total.to_string(),
        ),
    ]
}

fn start_location(response: &reqwest::Response) -> Result<String, GcsError> {
    let header = location_header(response)?;
    location_text(header)
}

fn location_header(
    response: &reqwest::Response,
) -> Result<&reqwest::header::HeaderValue, GcsError> {
    response
        .headers()
        .get(reqwest::header::LOCATION)
        .ok_or_else(|| {
            GcsError::new(
                GcsErrorKind::Api,
                "resumable upload did not return Location",
            )
        })
}

fn location_text(location: &reqwest::header::HeaderValue) -> Result<String, GcsError> {
    location
        .to_str()
        .map(|value| value.to_string())
        .map_err(|_| GcsError::new(GcsErrorKind::Api, "resumable Location header is not UTF-8"))
}

fn https_location(location: String) -> Result<String, GcsError> {
    if location.starts_with("https://") {
        Ok(location)
    } else {
        Err(GcsError::new(
            GcsErrorKind::Api,
            "resumable Location is not an https URL",
        ))
    }
}

fn part_spec(total: u64, offset: u64) -> PartSpec {
    let remaining = total - offset;
    PartSpec {
        is_last: remaining <= CHUNK_SIZE as u64,
        len: usize::try_from(remaining.min(CHUNK_SIZE as u64)).unwrap_or(CHUNK_SIZE),
    }
}

fn finish_part(step: Result<Part, GcsError>) -> Result<ObjectMeta, GcsError> {
    match step {
        Ok(part) => finish_ok(part),
        Err(err) => Err(err),
    }
}

fn finish_ok(part: Part) -> Result<ObjectMeta, GcsError> {
    if let Part::Done(meta) = part {
        return Ok(meta);
    }
    finish_stopped(part)
}

fn finish_stopped(part: Part) -> Result<ObjectMeta, GcsError> {
    if let Part::Finished = part {
        return Err(GcsError::new(
            GcsErrorKind::Api,
            "finished upload did not return object metadata",
        ));
    }
    Err(GcsError::new(
        GcsErrorKind::Api,
        "finished upload did not return object metadata",
    ))
}

fn follow_put(step: PutFollow) -> Attempt {
    if let PutFollow::Stop(result) = step {
        return Attempt::Stop(result);
    }
    follow_again(step)
}

fn follow_again(step: PutFollow) -> Attempt {
    if let PutFollow::Again { offset, attempt } = step {
        return Attempt::Again { offset, attempt };
    }
    Attempt::Again {
        offset: 0,
        attempt: 0,
    }
}

fn follow_upload_ok(step: UploadStep, start: u64, chunk_len: u64, offset: u64) -> PutFollow {
    match step {
        UploadStep::Done(meta) => PutFollow::Stop(Ok(UploadStep::Done(meta))),
        UploadStep::Continue { next } => continue_upload(next, start, chunk_len, offset),
    }
}

fn continue_upload(next: u64, start: u64, chunk_len: u64, offset: u64) -> PutFollow {
    if next >= start + chunk_len {
        return PutFollow::Stop(Ok(UploadStep::Continue { next }));
    }
    advance_upload(next, offset)
}

fn advance_upload(next: u64, offset: u64) -> PutFollow {
    if next > offset {
        PutFollow::Again {
            offset: next,
            attempt: 0,
        }
    } else {
        PutFollow::Probe
    }
}

fn follow_probe_ok(
    step: UploadStep,
    start: u64,
    chunk_len: u64,
    offset: u64,
    attempt: u32,
) -> Attempt {
    match step {
        UploadStep::Done(meta) => Attempt::Stop(Ok(UploadStep::Done(meta))),
        UploadStep::Continue { next } => probe_continue(next, start, chunk_len, offset, attempt),
    }
}

fn probe_continue(next: u64, start: u64, chunk_len: u64, offset: u64, attempt: u32) -> Attempt {
    if outside_chunk(next, start, chunk_len) {
        return Attempt::Stop(Ok(UploadStep::Continue { next }));
    }
    probe_inside(next, offset, attempt)
}

fn outside_chunk(next: u64, start: u64, chunk_len: u64) -> bool {
    next < start || next >= start + chunk_len
}

fn probe_inside(next: u64, offset: u64, attempt: u32) -> Attempt {
    if next > offset {
        return Attempt::Again {
            offset: next,
            attempt: 0,
        };
    }
    probe_rewind(next, offset, attempt)
}

fn probe_rewind(next: u64, offset: u64, attempt: u32) -> Attempt {
    if next < offset {
        Attempt::Again {
            offset: next,
            attempt,
        }
    } else {
        Attempt::Again { offset, attempt }
    }
}

enum UploadStep {
    Done(ObjectMeta),
    Continue { next: u64 },
}

impl UploadStep {
    async fn from_response(
        response: reqwest::Response,
        fallback_next: u64,
    ) -> Result<Self, GcsError> {
        if response.status().is_success() {
            return Ok(Self::Done(ObjectMeta::from_response(response).await?));
        }
        let header = response
            .headers()
            .get(reqwest::header::RANGE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let _ = response.bytes().await;
        let next = match header {
            Some(value) => next_offset_from_range(&value)?,
            None => fallback_next,
        };
        Ok(Self::Continue { next })
    }

    fn expect_done(self) -> Result<ObjectMeta, GcsError> {
        match self {
            Self::Done(meta) => Ok(meta),
            Self::Continue { .. } => Err(GcsError::new(
                GcsErrorKind::Api,
                "finished upload did not return object metadata",
            )),
        }
    }
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

fn next_offset_from_range(header: &str) -> Result<u64, GcsError> {
    let header = header.trim();
    let rest = header
        .strip_prefix("bytes=")
        .ok_or_else(|| GcsError::new(GcsErrorKind::Api, "resumable Range header is invalid"))?;
    let rest = rest.split(',').next().unwrap_or(rest).trim();
    let (start, end) = rest
        .split_once('-')
        .ok_or_else(|| GcsError::new(GcsErrorKind::Api, "resumable Range header is invalid"))?;
    let start: u64 = start
        .trim()
        .parse()
        .map_err(|_| GcsError::new(GcsErrorKind::Api, "resumable Range header is invalid"))?;
    let end: u64 = end
        .trim()
        .parse()
        .map_err(|_| GcsError::new(GcsErrorKind::Api, "resumable Range header is invalid"))?;
    if end < start {
        return Err(GcsError::new(
            GcsErrorKind::Api,
            "resumable Range header is invalid",
        ));
    }
    end.checked_add(1)
        .ok_or_else(|| GcsError::new(GcsErrorKind::Api, "resumable Range header is invalid"))
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use crate::gcp::{CachedToken, UserCredentials};
    use crate::gcs::gcs_client::credentials::Auth;

    use super::*;

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
        let auth = Auth::User {
            credentials: UserCredentials {
                client_id: "client".to_string(),
                client_secret: "secret".to_string(),
                refresh_token: "refresh".to_string(),
                quota_project_id: None,
            },
            signer_email: "signer@example.iam.gserviceaccount.com".to_string(),
        };
        let client = GcsClient::new("my-bucket", auth).await.unwrap();
        *client.inner.token.lock().await = Some(CachedToken {
            value: "token".to_string(),
            expires_at: Instant::now() + Duration::from_secs(600),
        });
        let upload = ResumableUpload {
            client: &client,
            location,
            content_type: "application/octet-stream",
            total: 0,
        };
        upload
            .put_chunk_retrying(b"", 0, true)
            .await
            .unwrap()
            .expect_done()
            .unwrap();
        let head = server.await.unwrap();
        assert!(head.contains("\r\ncontent-length: 0\r\n"), "{head}");
    }

    #[test]
    fn resume_range_is_the_end_plus_one() {
        assert_eq!(next_offset_from_range("bytes=0-42").unwrap(), 43);
        assert_eq!(next_offset_from_range("bytes=0-0").unwrap(), 1);
        assert!(next_offset_from_range("bytes=*").is_err());
        assert!(next_offset_from_range("0-1").is_err());
    }
}
