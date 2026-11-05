use std::collections::BTreeMap;
use std::time::Duration;

use crate::logging::LogLevel;

use crate::exception;
use crate::exception::Exception;
use crate::gcs::encode::encode_seg;
use crate::gcs::gcs_client::request::{logged_error, Call, OpsRecord};
use crate::gcs::gcs_client::upload::upload_source::UploadSource;
use crate::gcs::gcs_client::{GcsClient, ObjectMeta, TRANSFER_TIMEOUT};
use crate::gcs::is_retryable;

pub(crate) const CHUNK_SIZE: usize = 8 * 1024 * 1024;
pub(crate) const CHUNK_ALIGN: u64 = 256 * 1024;
pub(crate) const CHUNK_ATTEMPTS: u32 = 5;
const CHUNK_BACKOFF: Duration = Duration::from_millis(200);

pub(crate) struct ResumableUpload<'a> {
    pub(crate) client: &'a GcsClient,
    pub(crate) location: String,
    pub(crate) content_type: &'a str,
    pub(crate) total: u64,
}

impl<'a> ResumableUpload<'a> {
    pub(crate) async fn start(
        client: &'a GcsClient,
        name: &str,
        content_type: &'a str,
        metadata: &BTreeMap<String, String>,
        total: u64,
    ) -> Result<Self, Exception> {
        let url = format!(
            "{}/b/{}/o?uploadType=resumable",
            client.inner.endpoints.upload_root,
            encode_seg(client.bucket()),
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
                total.to_string(),
            ),
        ];
        let response = client
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
                exception!("resumable upload did not return Location", code = "GCS_API")
            })?;
        let location = location
            .to_str()
            .map_err(|_| exception!("resumable Location header is not UTF-8", code = "GCS_API"))?
            .to_string();
        let _ = response.bytes().await;
        // Every chunk request carries the access token, so the session URL must stay on the Google host.
        if !location.starts_with(&client.inner.endpoints.upload_location_prefix) {
            return Err(exception!(
                "resumable Location is not a storage.googleapis.com URL",
                code = "GCS_API"
            ));
        }
        Ok(Self {
            client,
            location,
            content_type,
            total,
        })
    }

    pub(crate) async fn run(&self, source: &mut UploadSource<'_>) -> Result<ObjectMeta, Exception> {
        let total = self.total;
        if total == 0 {
            return self.put_chunk_retrying(b"", 0, true).await?.expect_done();
        }
        let mut offset = 0u64;
        let mut rewinds = 0u32;
        while offset < total {
            let remaining = total - offset;
            let is_last = remaining <= CHUNK_SIZE as u64;
            let len = usize::try_from(remaining.min(CHUNK_SIZE as u64)).unwrap_or(CHUNK_SIZE);
            let chunk = source.read_chunk(offset, len).await?;
            match self.put_chunk_retrying(&chunk, offset, is_last).await? {
                UploadStep::Done(meta) => return Ok(meta),
                UploadStep::Continue { next } => {
                    advance_offset(&mut offset, &mut rewinds, next, total)?;
                }
            }
        }
        Err(exception!(
            "finished upload did not return object metadata",
            code = "GCS_API"
        ))
    }

    pub(crate) async fn put_chunk_retrying(
        &self,
        chunk: &[u8],
        start: u64,
        finished: bool,
    ) -> Result<UploadStep, Exception> {
        let chunk_len = chunk.len() as u64;
        let mut offset = start;
        let mut attempt = 0u32;
        loop {
            if chunk_suffix_len(chunk_len, start, offset, finished).is_none() {
                return Ok(UploadStep::Continue { next: offset });
            }
            attempt += 1;
            if attempt > CHUNK_ATTEMPTS {
                return Err(exception!(
                    "resumable chunk failed after retries",
                    code = "GCS_TRANSPORT"
                ));
            }
            if attempt > 1 {
                tokio::time::sleep(retry_delay(attempt - 1)).await;
            }
            match self.try_chunk(chunk, start, offset, attempt).await? {
                Move::Finish(step) => return Ok(step),
                Move::Reset(next) => {
                    offset = next;
                    attempt = 0;
                }
                Move::Set(next) => offset = next,
                Move::Stay => {}
            }
        }
    }

    async fn try_chunk(
        &self,
        chunk: &[u8],
        start: u64,
        offset: u64,
        attempt: u32,
    ) -> Result<Move, Exception> {
        let end = start + chunk.len() as u64;
        let piece = chunk_piece(chunk, start, offset)?;
        if let Some(done) = self.send_piece(piece, end, offset, attempt).await? {
            return Ok(done);
        }
        self.probe_chunk(start, end, offset, attempt).await
    }

    async fn send_piece(
        &self,
        piece: &[u8],
        end: u64,
        offset: u64,
        attempt: u32,
    ) -> Result<Option<Move>, Exception> {
        match self.upload_chunk_once(piece, offset).await {
            Ok(UploadStep::Done(meta)) => Ok(Some(Move::Finish(UploadStep::Done(meta)))),
            Ok(UploadStep::Continue { next }) if next >= end => {
                Ok(Some(Move::Finish(UploadStep::Continue { next })))
            }
            Ok(UploadStep::Continue { next }) if next > offset => Ok(Some(Move::Reset(next))),
            Ok(UploadStep::Continue { .. }) => Ok(None),
            Err(err) => {
                self.chunk_failure(err, "gcs resumable chunk failed", offset, attempt)?;
                Ok(None)
            }
        }
    }

    async fn probe_chunk(
        &self,
        start: u64,
        end: u64,
        offset: u64,
        attempt: u32,
    ) -> Result<Move, Exception> {
        match self.probe_session().await {
            Ok(UploadStep::Done(meta)) => Ok(Move::Finish(UploadStep::Done(meta))),
            Ok(UploadStep::Continue { next }) => Ok(after_probe(next, start, end, offset)),
            Err(err) => {
                self.chunk_failure(err, "gcs resumable probe failed", offset, attempt)?;
                Ok(Move::Stay)
            }
        }
    }

    fn chunk_failure(
        &self,
        err: Exception,
        message: &str,
        offset: u64,
        attempt: u32,
    ) -> Result<(), Exception> {
        if !is_retryable(&err) {
            return Err(err);
        }
        let attempt_text = attempt.to_string();
        let offset_text = offset.to_string();
        let error_text = logged_error(&String::from(&err));
        self.client.record_op(OpsRecord {
            level: LogLevel::Warn,
            message,
            method: "PUT",
            url: &self.location,
            status: None,
            latency_ms: 0,
            extra: &[
                ("attempt", &attempt_text),
                ("offset", &offset_text),
                ("error", &error_text),
            ],
        });
        Ok(())
    }

    async fn upload_chunk_once(&self, chunk: &[u8], offset: u64) -> Result<UploadStep, Exception> {
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
                no_retry: true,
                ..Default::default()
            })
            .await?;
        UploadStep::from_response(response, offset + chunk.len() as u64).await
    }

    async fn probe_session(&self) -> Result<UploadStep, Exception> {
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
                no_retry: true,
                ..Default::default()
            })
            .await?;
        UploadStep::from_response(response, 0).await
    }
}

pub(crate) enum Move {
    Finish(UploadStep),
    Reset(u64),
    Set(u64),
    Stay,
}

pub(crate) enum UploadStep {
    Done(ObjectMeta),
    Continue { next: u64 },
}

impl UploadStep {
    async fn from_response(
        response: reqwest::Response,
        fallback_next: u64,
    ) -> Result<Self, Exception> {
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

    pub(crate) fn expect_done(self) -> Result<ObjectMeta, Exception> {
        match self {
            Self::Done(meta) => Ok(meta),
            Self::Continue { .. } => Err(exception!(
                "finished upload did not return object metadata",
                code = "GCS_API"
            )),
        }
    }
}

fn chunk_piece(chunk: &[u8], start: u64, offset: u64) -> Result<&[u8], Exception> {
    usize::try_from(offset - start)
        .ok()
        .and_then(|from| chunk.get(from..))
        .ok_or_else(|| exception!("resumable offset is outside the chunk", code = "GCS_API"))
}

pub(crate) fn after_probe(next: u64, start: u64, end: u64, offset: u64) -> Move {
    if next < start || next >= end {
        return Move::Finish(UploadStep::Continue { next });
    }
    match next.cmp(&offset) {
        std::cmp::Ordering::Greater => Move::Reset(next),
        std::cmp::Ordering::Less => Move::Set(next),
        std::cmp::Ordering::Equal => Move::Stay,
    }
}

pub(crate) fn retry_delay(attempt: u32) -> Duration {
    CHUNK_BACKOFF.saturating_mul(u32::max(attempt, 1))
}

pub(crate) fn advance_offset(
    offset: &mut u64,
    rewinds: &mut u32,
    next: u64,
    total: u64,
) -> Result<(), Exception> {
    if next > total {
        return Err(exception!(
            "resumable offset is past the end of the object",
            code = "GCS_API"
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
            return Err(exception!(
                "resumable session did not keep uploaded bytes",
                code = "GCS_UNAVAILABLE"
            ));
        }
        *offset = next;
        return Ok(());
    }
    Err(exception!(
        "resumable upload did not advance",
        code = "GCS_API"
    ))
}

pub(crate) fn chunk_suffix_len(
    chunk_len: u64,
    start: u64,
    offset: u64,
    finished: bool,
) -> Option<u64> {
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

pub(crate) fn chunk_content_range(start: u64, len: u64, total: u64) -> String {
    if len == 0 {
        return format!("bytes */{total}");
    }
    let end = start + len - 1;
    format!("bytes {start}-{end}/{total}")
}

pub(crate) fn probe_content_range(total: u64) -> String {
    format!("bytes */{total}")
}

pub(crate) fn next_offset_from_range(header: &str) -> Result<u64, Exception> {
    range_end(header)
        .ok_or_else(|| exception!("resumable Range header is invalid", code = "GCS_API"))
}

fn range_end(header: &str) -> Option<u64> {
    let rest = header.trim().strip_prefix("bytes=")?;
    let rest = rest.split(',').next().unwrap_or(rest).trim();
    let (start, end) = rest.split_once('-')?;
    let start: u64 = start.trim().parse().ok()?;
    let end: u64 = end.trim().parse().ok()?;
    if end < start {
        return None;
    }
    end.checked_add(1)
}
