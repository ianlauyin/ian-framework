use std::collections::BTreeMap;
use std::time::Duration;

use crate::logging::LogLevel;

use crate::gcs::encode::encode_seg;
use crate::gcs::gcs_client::request::{logged_error, Call, OpsRecord};
use crate::gcs::gcs_client::upload::upload_source::UploadSource;
use crate::gcs::gcs_client::{GcsClient, ObjectMeta, TRANSFER_TIMEOUT};
use crate::gcs::{GcsError, GcsErrorKind};

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
        // Every chunk request carries the access token, so the session URL must stay on the Google host.
        if !location.starts_with(&client.inner.endpoints.upload_location_prefix) {
            return Err(GcsError::new(
                GcsErrorKind::Api,
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

    pub(crate) async fn run(&self, source: &mut UploadSource<'_>) -> Result<ObjectMeta, GcsError> {
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
        Err(GcsError::new(
            GcsErrorKind::Api,
            "finished upload did not return object metadata",
        ))
    }

    async fn put_chunk_retrying(
        &self,
        chunk: &[u8],
        start: u64,
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
    ) -> Result<Move, GcsError> {
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
    ) -> Result<Option<Move>, GcsError> {
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
    ) -> Result<Move, GcsError> {
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
        err: GcsError,
        message: &str,
        offset: u64,
        attempt: u32,
    ) -> Result<(), GcsError> {
        if !err.is_retryable() {
            return Err(err);
        }
        let attempt_text = attempt.to_string();
        let offset_text = offset.to_string();
        let error_text = logged_error(&err);
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
        Ok(())
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
                no_retry: true,
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
                no_retry: true,
                ..Default::default()
            })
            .await?;
        UploadStep::from_response(response, 0).await
    }
}

enum Move {
    Finish(UploadStep),
    Reset(u64),
    Set(u64),
    Stay,
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

fn chunk_piece(chunk: &[u8], start: u64, offset: u64) -> Result<&[u8], GcsError> {
    usize::try_from(offset - start)
        .ok()
        .and_then(|from| chunk.get(from..))
        .ok_or_else(|| GcsError::new(GcsErrorKind::Api, "resumable offset is outside the chunk"))
}

fn after_probe(next: u64, start: u64, end: u64, offset: u64) -> Move {
    if next < start || next >= end {
        return Move::Finish(UploadStep::Continue { next });
    }
    match next.cmp(&offset) {
        std::cmp::Ordering::Greater => Move::Reset(next),
        std::cmp::Ordering::Less => Move::Set(next),
        std::cmp::Ordering::Equal => Move::Stay,
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
    range_end(header)
        .ok_or_else(|| GcsError::new(GcsErrorKind::Api, "resumable Range header is invalid"))
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

#[cfg(test)]
mod tests {
    use crate::gcs::gcs_client::tests::{fake_client, serve_script, Reply};
    #[test]
    fn the_session_url_must_stay_on_the_google_host() {
        let prefix = Endpoints::google().upload_location_prefix;
        assert!(
            "https://storage.googleapis.com/upload/storage/v1/b/my-app/o?upload_id=x"
                .starts_with(&prefix)
        );
        assert!(!"https://evil.example/upload".starts_with(&prefix));
        assert!(!"http://storage.googleapis.com/upload".starts_with(&prefix));
        assert!(!"https://storage.googleapis.com.evil.example/upload".starts_with(&prefix));
        assert!(!"https://storage.googleapis.com@evil.example/".starts_with(&prefix));
    }

    use std::time::Instant;

    use crate::gcp::{CachedToken, UserCredentials};
    use crate::gcs::gcs_client::credentials::Auth;
    use crate::gcs::gcs_client::{Endpoints, PutBody};

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

    const META: &str = r#"{"name":"a","size":"10"}"#;

    fn session_reply() -> Reply {
        Reply::new(200, "").header("location", "http://{addr}/session")
    }

    fn range_reply(range: &str) -> Reply {
        Reply::new(308, "").header("range", range)
    }

    fn upload_to(client: &GcsClient, total: u64) -> ResumableUpload<'_> {
        ResumableUpload {
            client,
            location: format!("{}session", client.inner.endpoints.upload_location_prefix),
            content_type: "text/plain",
            total,
        }
    }

    async fn start_with(reply: Reply) -> Result<(), GcsError> {
        let (addr, _server) = serve_script(vec![reply]).await;
        let client = fake_client(addr);
        let metadata = BTreeMap::new();
        ResumableUpload::start(&client, "a", "text/plain", &metadata, 1)
            .await
            .map(|_| ())
    }

    #[tokio::test]
    async fn start_refuses_a_missing_or_foreign_session_url() {
        let missing = start_with(Reply::new(200, "")).await.unwrap_err();
        assert!(missing.to_string().contains("did not return Location"));
        let foreign = Reply::new(200, "").header("location", "https://evil.example/session");
        let err = start_with(foreign).await.unwrap_err();
        assert!(err.to_string().contains("not a storage.googleapis.com URL"));
        let not_utf8 = Reply::new(200, "").header("location", [0xff, 0xfe]);
        let err = start_with(not_utf8).await.unwrap_err();
        assert!(err.to_string().contains("not UTF-8"), "{err}");
        start_with(session_reply()).await.unwrap();
    }

    #[tokio::test]
    async fn run_sends_one_chunk_and_finishes() {
        let (addr, server) = serve_script(vec![Reply::new(200, META)]).await;
        let client = fake_client(addr);
        let body = PutBody::Bytes(b"0123456789".to_vec());
        let mut source = UploadSource::open(&body).await.unwrap();
        let meta = upload_to(&client, 10).run(&mut source).await.unwrap();
        assert_eq!(meta.size, 10);
        let request = server.await.unwrap().remove(0);
        assert!(request
            .to_ascii_lowercase()
            .contains("content-range: bytes 0-9/10"));
    }

    #[tokio::test]
    async fn run_sends_a_second_chunk_after_the_range_of_the_first() {
        let total = CHUNK_SIZE + 1;
        let first = range_reply(&format!("bytes=0-{}", CHUNK_SIZE - 1));
        let (addr, server) = serve_script(vec![first, Reply::new(200, META)]).await;
        let client = fake_client(addr);
        let data = vec![7_u8; total];
        let body = PutBody::Bytes(data);
        let mut source = UploadSource::open(&body).await.unwrap();
        upload_to(&client, total as u64)
            .run(&mut source)
            .await
            .unwrap();
        let requests = server.await.unwrap();
        let second = requests[1].to_ascii_lowercase();
        assert!(second.contains(&format!(
            "content-range: bytes {CHUNK_SIZE}-{CHUNK_SIZE}/{total}"
        )));
    }

    #[tokio::test]
    async fn run_fails_when_the_last_chunk_returns_no_metadata() {
        let (addr, _server) = serve_script(vec![range_reply("bytes=0-9")]).await;
        let client = fake_client(addr);
        let body = PutBody::Bytes(b"0123456789".to_vec());
        let mut source = UploadSource::open(&body).await.unwrap();
        let err = upload_to(&client, 10).run(&mut source).await.unwrap_err();
        assert!(err.to_string().contains("did not return object metadata"));
    }

    #[tokio::test]
    async fn run_fails_when_an_empty_object_returns_no_metadata() {
        let (addr, _server) = serve_script(vec![range_reply("bytes=0-0")]).await;
        let client = fake_client(addr);
        let body = PutBody::Bytes(Vec::new());
        let mut source = UploadSource::open(&body).await.unwrap();
        assert!(upload_to(&client, 0).run(&mut source).await.is_err());
    }

    async fn put_chunk(replies: Vec<Reply>, start: u64) -> Result<UploadStep, GcsError> {
        let (addr, _server) = serve_script(replies).await;
        let client = fake_client(addr);
        upload_to(&client, 20)
            .put_chunk_retrying(b"0123456789", start, true)
            .await
    }

    fn is_done(step: Result<UploadStep, GcsError>) -> bool {
        matches!(step, Ok(UploadStep::Done(_)))
    }

    #[tokio::test]
    async fn a_failed_chunk_resumes_from_the_offset_the_session_reports() {
        let replies = vec![
            Reply::new(500, "{}"),
            range_reply("bytes=0-7"),
            Reply::new(200, META),
        ];
        let (addr, server) = serve_script(replies).await;
        let client = fake_client(addr);
        let step = upload_to(&client, 10)
            .put_chunk_retrying(b"0123456789", 0, true)
            .await;
        assert!(is_done(step));
        let requests = server.await.unwrap();
        assert!(requests[1]
            .to_ascii_lowercase()
            .contains("content-range: bytes */10"));
        assert!(requests[2]
            .to_ascii_lowercase()
            .contains("content-range: bytes 8-9/10"));
        assert!(requests[2].ends_with("89"));
    }

    #[tokio::test]
    async fn a_chunk_that_made_no_progress_is_sent_again() {
        let replies = vec![
            Reply::new(500, "{}"),
            Reply::new(308, ""),
            Reply::new(200, META),
        ];
        assert!(is_done(put_chunk(replies, 0).await));
    }

    #[tokio::test]
    async fn the_session_can_move_the_offset_back_inside_the_chunk() {
        let replies = vec![
            Reply::new(500, "{}"),
            range_reply("bytes=0-13"),
            Reply::new(500, "{}"),
            range_reply("bytes=0-11"),
            Reply::new(200, META),
        ];
        assert!(is_done(put_chunk(replies, 10).await));
    }

    #[tokio::test]
    async fn a_probe_that_finds_the_finished_object_ends_the_upload() {
        let replies = vec![Reply::new(500, "{}"), Reply::new(200, META)];
        assert!(is_done(put_chunk(replies, 0).await));
    }

    #[tokio::test]
    async fn a_failed_probe_is_retried_but_a_final_error_is_returned() {
        let replies = vec![
            Reply::new(500, "{}"),
            Reply::new(503, "{}"),
            Reply::new(200, META),
        ];
        assert!(is_done(put_chunk(replies, 0).await));
        let replies = vec![Reply::new(500, "{}"), Reply::new(403, "{}")];
        let err = put_chunk(replies, 0).await.map(|_| ()).unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Auth);
    }

    #[tokio::test]
    async fn a_chunk_error_that_cannot_be_retried_is_returned() {
        let err = put_chunk(vec![Reply::new(403, "{}")], 0)
            .await
            .map(|_| ())
            .unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Auth);
    }

    #[tokio::test]
    async fn a_range_that_covers_the_chunk_ends_the_attempt() {
        let step = put_chunk(vec![range_reply("bytes=0-19")], 10)
            .await
            .unwrap();
        assert!(matches!(step, UploadStep::Continue { next: 20 }));
    }

    #[tokio::test]
    async fn a_response_without_a_range_keeps_the_sent_offset() {
        let step = put_chunk(vec![Reply::new(308, "")], 10).await.unwrap();
        assert!(matches!(step, UploadStep::Continue { next: 20 }));
    }

    #[tokio::test]
    async fn a_bad_range_header_is_an_api_error() {
        let err = put_chunk(vec![range_reply("bytes=x")], 0)
            .await
            .map(|_| ())
            .unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Api);
    }

    #[test]
    fn a_bad_range_header_is_refused() {
        for header in [
            "bytes=a-1",
            "bytes=1-b",
            "bytes=5-2",
            "bytes=7",
            "bytes=0-18446744073709551615",
        ] {
            assert!(next_offset_from_range(header).is_err(), "{header}");
        }
        assert_eq!(next_offset_from_range(" bytes=0-4, 6-9 ").unwrap(), 5);
    }

    #[test]
    fn a_probe_answer_outside_the_chunk_ends_the_attempt() {
        assert!(matches!(after_probe(4, 5, 15, 5), Move::Finish(_)));
        assert!(matches!(after_probe(15, 5, 15, 5), Move::Finish(_)));
        assert!(matches!(after_probe(9, 5, 15, 5), Move::Reset(9)));
        assert!(matches!(after_probe(6, 5, 15, 8), Move::Set(6)));
        assert!(matches!(after_probe(8, 5, 15, 8), Move::Stay));
    }
}
