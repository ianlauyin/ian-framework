use std::collections::BTreeMap;
use std::time::Duration;

use crate::logging::LogLevel;

use crate::gcs::encode::encode_seg;
use crate::gcs::gcs_client::{logged_error, Call, GcsClient, OpsRecord, TRANSFER_TIMEOUT};
use crate::gcs::{GcsError, GcsErrorKind, ObjectMeta};

pub(super) const CHUNK_SIZE: usize = 8 * 1024 * 1024;
pub(super) const CHUNK_ALIGN: u64 = 256 * 1024;
const CHUNK_ATTEMPTS: u32 = 5;
const CHUNK_BACKOFF: Duration = Duration::from_millis(200);
const UPLOAD_ROOT: &str = "https://storage.googleapis.com/upload/storage/v1";

pub(super) enum UploadStep {
    Done(ObjectMeta),
    Continue { next: u64 },
}

impl UploadStep {
    pub(super) fn expect_done(self) -> Result<ObjectMeta, GcsError> {
        match self {
            Self::Done(meta) => Ok(meta),
            Self::Continue { .. } => Err(GcsError::new(
                GcsErrorKind::Api,
                "finished upload did not return object metadata",
            )),
        }
    }
}

impl GcsClient {
    pub(super) async fn start_resumable(
        &self,
        name: &str,
        content_type: &str,
        metadata: &BTreeMap<String, String>,
        total_len: Option<u64>,
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
            body["metadata"] = serde_json::to_value(metadata).map_err(|err| {
                GcsError::with_source(GcsErrorKind::Config, "metadata is not JSON", err)
            })?;
        }
        let mut headers = vec![(
            reqwest::header::HeaderName::from_static("x-upload-content-type"),
            content_type.to_string(),
        )];
        if let Some(len) = total_len {
            headers.push((
                reqwest::header::HeaderName::from_static("x-upload-content-length"),
                len.to_string(),
            ));
        }
        let response = self
            .send(&Call {
                method: reqwest::Method::POST,
                url,
                headers,
                json: Some(body),
                bytes: None,
                extra_status: &[],
                timeout: None,
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

    pub(super) async fn put_chunk_retrying(
        &self,
        location: &str,
        content_type: &str,
        chunk: &[u8],
        start: u64,
        total: Option<u64>,
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
                Ok(Sent::Done(meta)) => return Ok(UploadStep::Done(meta)),
                Ok(Sent::Accepted { next }) => {
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
                Ok(Probe::Complete(meta)) => return Ok(UploadStep::Done(meta)),
                Ok(Probe::Incomplete { next }) => {
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
        total: Option<u64>,
    ) -> Result<Sent, GcsError> {
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
                json: None,
                bytes: Some(chunk.to_vec()),
                extra_status: &[308],
                timeout: Some(TRANSFER_TIMEOUT),
            })
            .await?;
        if response.status().is_success() {
            return Ok(Sent::Done(ObjectMeta::from_response(response).await?));
        }
        let header = response
            .headers()
            .get(reqwest::header::RANGE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let _ = response.bytes().await;
        let next = match header {
            Some(value) => next_offset_from_range(&value)?,
            None => offset + chunk.len() as u64,
        };
        Ok(Sent::Accepted { next })
    }

    async fn probe_session(&self, location: &str, total: Option<u64>) -> Result<Probe, GcsError> {
        let response = self
            .send(&Call {
                method: reqwest::Method::PUT,
                url: location.to_string(),
                headers: vec![
                    (reqwest::header::CONTENT_RANGE, probe_content_range(total)),
                    (reqwest::header::CONTENT_LENGTH, "0".to_string()),
                ],
                json: None,
                bytes: Some(Vec::new()),
                extra_status: &[308],
                timeout: Some(TRANSFER_TIMEOUT),
            })
            .await?;
        if response.status().is_success() {
            return Ok(Probe::Complete(ObjectMeta::from_response(response).await?));
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
        Ok(Probe::Incomplete { next })
    }
}

enum Sent {
    Done(ObjectMeta),
    Accepted { next: u64 },
}

enum Probe {
    Complete(ObjectMeta),
    Incomplete { next: u64 },
}

fn retry_delay(attempt: u32) -> Duration {
    CHUNK_BACKOFF.saturating_mul(u32::max(attempt, 1))
}

pub(super) fn advance_offset(
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

fn chunk_content_range(start: u64, len: u64, total: Option<u64>) -> String {
    if len == 0 {
        return format!("bytes */{}", total.unwrap_or(0));
    }
    let end = start + len - 1;
    match total {
        Some(total) => format!("bytes {start}-{end}/{total}"),
        None => format!("bytes {start}-{end}/*"),
    }
}

fn probe_content_range(total: Option<u64>) -> String {
    match total {
        Some(total) => format!("bytes */{total}"),
        None => "bytes */*".to_string(),
    }
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
    use super::*;

    #[test]
    fn probe_and_chunk_ranges_match_the_upload_shape() {
        assert_eq!(probe_content_range(Some(0)), "bytes */0");
        assert_eq!(probe_content_range(Some(12)), "bytes */12");
        assert_eq!(probe_content_range(None), "bytes */*");
        assert_eq!(chunk_content_range(0, 0, Some(0)), "bytes */0");
        assert_eq!(chunk_content_range(0, 4, Some(4)), "bytes 0-3/4");
        assert_eq!(chunk_content_range(8, 4, None), "bytes 8-11/*");
    }

    #[test]
    fn resume_range_is_the_end_plus_one() {
        assert_eq!(next_offset_from_range("bytes=0-42").unwrap(), 43);
        assert_eq!(next_offset_from_range("bytes=0-0").unwrap(), 1);
        assert!(next_offset_from_range("bytes=*").is_err());
        assert!(next_offset_from_range("0-1").is_err());
    }

    #[test]
    fn non_final_suffix_must_be_a_multiple_of_256_kib() {
        let align = CHUNK_ALIGN;
        assert_eq!(chunk_suffix_len(align, 0, 0, false), Some(align));
        assert_eq!(chunk_suffix_len(align, 0, 1, false), None);
        assert_eq!(chunk_suffix_len(align, 0, 1, true), Some(align - 1));
        assert_eq!(chunk_suffix_len(0, 0, 0, true), Some(0));
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
            .put_chunk_retrying(&location, "application/octet-stream", b"", 0, Some(0), true)
            .await
            .unwrap()
            .expect_done()
            .unwrap();
        let head = server.await.unwrap();
        assert!(head.contains("\r\ncontent-length: 0\r\n"), "{head}");
    }
}
