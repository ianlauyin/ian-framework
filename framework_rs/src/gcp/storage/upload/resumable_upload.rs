use std::borrow::Cow;
use std::collections::BTreeMap;
use std::time::Duration;

use crate::exception;
use crate::exception::Exception;
use crate::gcp::storage::call::Call;
use crate::gcp::storage::encode::encode_seg;
use crate::gcp::storage::error::is_retryable;
use crate::gcp::storage::ops_log::{logged_error, OpsRecord};
use crate::gcp::storage::upload::UploadSource;
use crate::gcp::storage::{GcsClient, ObjectMeta, TRANSFER_TIMEOUT};
use crate::logging::LogLevel;

pub(crate) const CHUNK_SIZE: usize = 8 * 1024 * 1024;
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
                exception!(code = "GCP_API", "resumable upload did not return Location")
            })?;
        let location = location
            .to_str()
            .map_err(|_| exception!(code = "GCP_API", "resumable Location header is not UTF-8"))?
            .to_string();
        let _ = response.bytes().await;
        // Every chunk request carries the access token, so the session URL must stay on the Google host.
        if !location.starts_with(&client.inner.endpoints.upload_location_prefix) {
            return Err(exception!(
                code = "GCP_API",
                "resumable Location is not a storage.googleapis.com URL",
            ));
        }
        Ok(Self {
            client,
            location,
            content_type,
            total,
        })
    }

    // After a failure, the next request is a probe: a chunk request with an empty body.
    pub(crate) async fn run(&self, source: &mut UploadSource<'_>) -> Result<ObjectMeta, Exception> {
        let mut offset = 0u64;
        let mut failures = 0u32;
        let mut rewinds = 0u32;
        let mut probe = false;
        loop {
            let chunk = if probe {
                Cow::Borrowed(&[][..])
            } else {
                let len = (self.total - offset).min(CHUNK_SIZE as u64);
                source
                    .read_chunk(offset, usize::try_from(len).unwrap_or(CHUNK_SIZE))
                    .await?
            };
            let next = match self.put_chunk(&chunk, offset).await {
                Ok(UploadStep::Done(meta)) => return Ok(meta),
                Ok(UploadStep::Continue { next }) => next,
                Err(err) => {
                    failures += 1;
                    self.chunk_failure(err, probe, offset, failures)?;
                    tokio::time::sleep(retry_delay(failures)).await;
                    probe = true;
                    continue;
                }
            };
            // The session kept no byte after the failure, so the chunk is sent again.
            if probe && next == offset {
                probe = false;
                continue;
            }
            if next > offset {
                failures = 0;
            }
            advance_offset(&mut offset, &mut rewinds, next, self.total)?;
            probe = false;
        }
    }

    fn chunk_failure(
        &self,
        err: Exception,
        probe: bool,
        offset: u64,
        failures: u32,
    ) -> Result<(), Exception> {
        if !is_retryable(&err) || failures >= CHUNK_ATTEMPTS {
            return Err(err);
        }
        let message = if probe {
            "gcs resumable probe failed"
        } else {
            "gcs resumable chunk failed"
        };
        let attempt_text = failures.to_string();
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

    async fn put_chunk(&self, chunk: &[u8], offset: u64) -> Result<UploadStep, Exception> {
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
        UploadStep::from_response(response).await
    }
}

pub(crate) enum UploadStep {
    Done(ObjectMeta),
    Continue { next: u64 },
}

impl UploadStep {
    // A 308 without a Range header means that GCS has kept no bytes of the session.
    async fn from_response(response: reqwest::Response) -> Result<Self, Exception> {
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
            None => 0,
        };
        Ok(Self::Continue { next })
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
            code = "GCP_API",
            "resumable offset is past the end of the object",
        ));
    }
    if next == total {
        return Err(exception!(
            code = "GCP_API",
            "finished upload did not return object metadata",
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
                code = "GCS_UNAVAILABLE",
                "resumable session did not keep uploaded bytes",
            ));
        }
        *offset = next;
        return Ok(());
    }
    Err(exception!(
        code = "GCP_API",
        "resumable upload did not advance",
    ))
}

pub(crate) fn chunk_content_range(start: u64, len: u64, total: u64) -> String {
    if len == 0 {
        return format!("bytes */{total}");
    }
    let end = start + len - 1;
    format!("bytes {start}-{end}/{total}")
}

pub(crate) fn next_offset_from_range(header: &str) -> Result<u64, Exception> {
    range_end(header)
        .ok_or_else(|| exception!(code = "GCP_API", "resumable Range header is invalid"))
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
