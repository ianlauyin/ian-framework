use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::exception::Exception;
use crate::gcp::credentials::CachedToken;
use crate::gcp::storage::error::is_retryable;
use crate::gcp::storage::upload::resumable_upload::*;
use crate::gcp::storage::upload::UploadSource;
use crate::gcp::storage::{Endpoints, GcsClient, ObjectMeta, PutBody, TRANSFER_TIMEOUT};
use crate::test::gcp::storage::{fake_client, user_credentials, SIGNER};
use crate::test::gcp::{serve_script, Reply};

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

#[test]
fn chunk_ranges_match_the_upload_shape() {
    assert_eq!(chunk_content_range(0, 0, 0), "bytes */0");
    assert_eq!(chunk_content_range(8, 0, 8), "bytes */8");
    assert_eq!(chunk_content_range(0, 4, 4), "bytes 0-3/4");
}

#[test]
fn advance_offset_rewinds_then_resets_after_progress() {
    let mut offset = 10u64;
    let mut rewinds = 0u32;
    advance_offset(&mut offset, &mut rewinds, 4, 100).unwrap();
    assert_eq!(offset, 4);
    assert_eq!(rewinds, 1);
    let stalled = advance_offset(&mut offset, &mut rewinds, 4, 100).unwrap_err();
    assert_eq!(stalled.code(), "GCP_API");
    advance_offset(&mut offset, &mut rewinds, 20, 100).unwrap();
    assert_eq!(offset, 20);
    assert_eq!(rewinds, 0);
    let past = advance_offset(&mut offset, &mut rewinds, 101, 100).unwrap_err();
    assert_eq!(past.code(), "GCP_API");
    let no_metadata = advance_offset(&mut offset, &mut rewinds, 100, 100).unwrap_err();
    assert!(no_metadata
        .to_string()
        .contains("did not return object metadata"));
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
    assert_eq!(err.code(), "GCS_UNAVAILABLE");
    assert!(is_retryable(&err));
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
    let client = GcsClient::new("my-bucket", user_credentials(), Some(SIGNER))
        .await
        .unwrap();
    *client.inner.tokens.cached.lock().await = Some(CachedToken {
        value: "token".to_string(),
        expires_at: Instant::now() + Duration::from_secs(600),
    });
    let upload = ResumableUpload {
        client: &client,
        location,
        content_type: "application/octet-stream",
        total: 0,
    };
    let body = PutBody::Bytes(Vec::new());
    let mut source = UploadSource::open(&body).await.unwrap();
    upload.run(&mut source).await.unwrap();
    let head = server.await.unwrap();
    assert!(head.contains("\r\ncontent-length: 0\r\n"), "{head}");
    assert!(head.contains("\r\ncontent-range: bytes */0\r\n"), "{head}");
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

async fn start_with(reply: Reply) -> Result<(), Exception> {
    let (addr, _server) = serve_script(vec![reply]).await;
    let client = fake_client(addr);
    let metadata = BTreeMap::new();
    ResumableUpload::start(&client, "a", "text/plain", &metadata, 1)
        .await
        .map(|_| ())
}

// The server must answer every request of the upload with one reply, so the script ends with the upload.
async fn run_script(replies: Vec<Reply>) -> (Result<ObjectMeta, Exception>, Vec<String>) {
    let (addr, server) = serve_script(replies).await;
    let client = fake_client(addr);
    let body = PutBody::Bytes(b"0123456789".to_vec());
    let mut source = UploadSource::open(&body).await.unwrap();
    let result = upload_to(&client, 10).run(&mut source).await;
    (result, server.await.unwrap())
}

fn content_range(request: &str) -> String {
    request
        .to_ascii_lowercase()
        .lines()
        .find_map(|line| line.strip_prefix("content-range: "))
        .unwrap_or_default()
        .trim()
        .to_string()
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
    let (result, requests) = run_script(vec![Reply::new(200, META)]).await;
    assert_eq!(result.unwrap().size, 10);
    assert_eq!(content_range(&requests[0]), "bytes 0-9/10");
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
    assert_eq!(
        content_range(&requests[1]),
        format!("bytes {CHUNK_SIZE}-{CHUNK_SIZE}/{total}")
    );
}

#[tokio::test]
async fn run_fails_when_the_last_chunk_returns_no_metadata() {
    let (result, _) = run_script(vec![range_reply("bytes=0-9")]).await;
    let err = result.unwrap_err();
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

#[tokio::test]
async fn a_failed_chunk_resumes_from_the_offset_the_session_reports() {
    let replies = vec![
        Reply::new(500, "{}"),
        range_reply("bytes=0-7"),
        Reply::new(200, META),
    ];
    let (result, requests) = run_script(replies).await;
    result.unwrap();
    assert_eq!(content_range(&requests[1]), "bytes */10");
    assert!(requests[1].ends_with("\r\n\r\n"));
    assert_eq!(content_range(&requests[2]), "bytes 8-9/10");
    assert!(requests[2].ends_with("89"));
}

#[tokio::test]
async fn a_chunk_that_made_no_progress_is_sent_again() {
    let replies = vec![
        Reply::new(500, "{}"),
        Reply::new(308, ""),
        Reply::new(200, META),
    ];
    let (result, requests) = run_script(replies).await;
    result.unwrap();
    assert_eq!(content_range(&requests[2]), "bytes 0-9/10");
}

#[tokio::test]
async fn the_session_can_move_the_offset_back() {
    let replies = vec![
        Reply::new(500, "{}"),
        range_reply("bytes=0-7"),
        Reply::new(500, "{}"),
        range_reply("bytes=0-5"),
        Reply::new(200, META),
    ];
    let (result, requests) = run_script(replies).await;
    result.unwrap();
    assert_eq!(content_range(&requests[4]), "bytes 6-9/10");
}

#[tokio::test]
async fn a_probe_that_finds_the_finished_object_ends_the_upload() {
    let (result, _) = run_script(vec![Reply::new(500, "{}"), Reply::new(200, META)]).await;
    result.unwrap();
}

#[tokio::test]
async fn a_failed_probe_is_retried_but_a_final_error_is_returned() {
    let replies = vec![
        Reply::new(500, "{}"),
        Reply::new(503, "{}"),
        Reply::new(200, META),
    ];
    run_script(replies).await.0.unwrap();
    let replies = vec![Reply::new(500, "{}"), Reply::new(403, "{}")];
    let err = run_script(replies).await.0.unwrap_err();
    assert_eq!(err.code(), "GCP_AUTH");
}

#[tokio::test]
async fn a_chunk_error_that_cannot_be_retried_is_returned() {
    let err = run_script(vec![Reply::new(403, "{}")]).await.0.unwrap_err();
    assert_eq!(err.code(), "GCP_AUTH");
}

#[tokio::test]
async fn the_upload_stops_after_the_attempt_limit() {
    let replies = (0..CHUNK_ATTEMPTS).map(|_| Reply::new(500, "{}")).collect();
    let (result, requests) = run_script(replies).await;
    assert_eq!(result.unwrap_err().code(), "GCS_UNAVAILABLE");
    assert_eq!(requests.len() as u32, CHUNK_ATTEMPTS);
}

#[tokio::test]
async fn a_chunk_response_that_does_not_advance_is_an_api_error() {
    let err = run_script(vec![Reply::new(308, "")]).await.0.unwrap_err();
    assert_eq!(err.code(), "GCP_API");
    assert!(err.to_string().contains("did not advance"));
}

#[tokio::test]
async fn a_response_without_a_range_means_that_no_bytes_are_kept() {
    let total = CHUNK_SIZE + 1;
    let first = range_reply(&format!("bytes=0-{}", CHUNK_SIZE - 1));
    let replies = vec![first, Reply::new(308, ""), Reply::new(200, META)];
    let (addr, server) = serve_script(replies).await;
    let client = fake_client(addr);
    let body = PutBody::Bytes(vec![7_u8; total]);
    let mut source = UploadSource::open(&body).await.unwrap();
    upload_to(&client, total as u64)
        .run(&mut source)
        .await
        .unwrap();
    let requests = server.await.unwrap();
    assert_eq!(
        content_range(&requests[2]),
        format!("bytes 0-{}/{total}", CHUNK_SIZE - 1)
    );
}

#[tokio::test]
async fn a_bad_range_header_is_an_api_error() {
    let err = run_script(vec![range_reply("bytes=x")])
        .await
        .0
        .unwrap_err();
    assert_eq!(err.code(), "GCP_API");
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
