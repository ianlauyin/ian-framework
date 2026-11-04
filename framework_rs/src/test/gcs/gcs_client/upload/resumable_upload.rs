use std::collections::BTreeMap;
use std::time::Duration;

use crate::exception::Exception;
use crate::gcs::gcs_client::upload::upload_source::UploadSource;
use crate::gcs::gcs_client::{GcsClient, TRANSFER_TIMEOUT};
use crate::gcs::is_retryable;

use crate::test::gcs::gcs_client::{fake_client, serve_script, Reply};
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

use crate::gcs::gcs_client::upload::resumable_upload::*;

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
    assert_eq!(stalled.code(), "GCS_API");
    advance_offset(&mut offset, &mut rewinds, 20, 100).unwrap();
    assert_eq!(offset, 20);
    assert_eq!(rewinds, 0);
    let past = advance_offset(&mut offset, &mut rewinds, 101, 100).unwrap_err();
    assert_eq!(past.code(), "GCS_API");
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

async fn start_with(reply: Reply) -> Result<(), Exception> {
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

async fn put_chunk(replies: Vec<Reply>, start: u64) -> Result<UploadStep, Exception> {
    let (addr, _server) = serve_script(replies).await;
    let client = fake_client(addr);
    upload_to(&client, 20)
        .put_chunk_retrying(b"0123456789", start, true)
        .await
}

fn is_done(step: Result<UploadStep, Exception>) -> bool {
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
    assert_eq!(err.code(), "GCS_AUTH");
}

#[tokio::test]
async fn a_chunk_error_that_cannot_be_retried_is_returned() {
    let err = put_chunk(vec![Reply::new(403, "{}")], 0)
        .await
        .map(|_| ())
        .unwrap_err();
    assert_eq!(err.code(), "GCS_AUTH");
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
    assert_eq!(err.code(), "GCS_API");
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
