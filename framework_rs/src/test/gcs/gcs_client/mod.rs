mod credentials;
mod object;
mod request;
mod rewrite_step;
mod upload;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::exception;
use crate::exception::Exception;
use crate::gcp::CachedToken;
use crate::gcs::gcs_client::credentials::Auth;
use crate::gcs::gcs_client::request::{Call, OpsRecord};
use crate::gcs::is_retryable_status;
use crate::logging::{LogLevel, LogShipper};
use std::net::SocketAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinHandle;

use crate::gcp::UserCredentials;

use crate::gcs::gcs_client::*;
use crate::test::logging::{buffered_events, shipper_config};

#[test]
fn object_names_are_checked() {
    assert!(validate_object_name("a/b.txt").is_ok());
    assert!(validate_object_name(&"a".repeat(MAX_OBJECT_NAME_BYTES)).is_ok());
    for bad in ["", ".", "..", "a\nb"] {
        assert!(validate_object_name(bad).is_err());
    }
    assert!(validate_object_name(&"a".repeat(MAX_OBJECT_NAME_BYTES + 1)).is_err());
}

#[test]
fn send_retries_only_a_transient_status_and_stops_at_the_limit() {
    for status in [408_u16, 429, 500, 502, 503, 504] {
        assert!(is_retryable_status(status));
    }
    for status in [200_u16, 400, 401, 403, 404, 412] {
        assert!(!is_retryable_status(status));
    }
    let call = Call::default();
    assert!(may_retry(&call, 0));
    assert!(may_retry(&call, SEND_ATTEMPTS - 2));
    assert!(!may_retry(&call, SEND_ATTEMPTS - 1));
    let own_loop = Call {
        no_retry: true,
        ..Call::default()
    };
    assert!(!may_retry(&own_loop, 0));
}

pub(crate) fn user_auth() -> Auth {
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

pub(crate) struct Reply {
    status: u16,
    headers: Vec<(String, Vec<u8>)>,
    body: Vec<u8>,
    declared_length: Option<usize>,
}

impl Reply {
    pub(crate) fn new(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.as_bytes().to_vec(),
            declared_length: None,
        }
    }

    pub(crate) fn header(mut self, name: &str, value: impl AsRef<[u8]>) -> Self {
        self.headers
            .push((name.to_string(), value.as_ref().to_vec()));
        self
    }

    pub(crate) fn declared_length(mut self, length: usize) -> Self {
        self.declared_length = Some(length);
        self
    }

    fn into_bytes(self, addr: SocketAddr) -> Vec<u8> {
        let length = self.declared_length.unwrap_or(self.body.len());
        let mut out = format!(
            "HTTP/1.1 {} X\r\ncontent-type: application/json\r\ncontent-length: {length}\r\nconnection: close\r\n",
            self.status
        )
        .into_bytes();
        for (name, value) in self.headers {
            out.extend_from_slice(format!("{name}: ").as_bytes());
            out.extend_from_slice(&replace_addr(&value, addr));
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(b"\r\n");
        out.extend_from_slice(&replace_addr(&self.body, addr));
        out
    }
}

fn replace_addr(bytes: &[u8], addr: SocketAddr) -> Vec<u8> {
    let placeholder = b"{addr}";
    let mut out = Vec::new();
    let mut rest = bytes;
    while let Some(at) = rest
        .windows(placeholder.len())
        .position(|window| window == placeholder)
    {
        out.extend_from_slice(&rest[..at]);
        out.extend_from_slice(addr.to_string().as_bytes());
        rest = &rest[at + placeholder.len()..];
    }
    out.extend_from_slice(rest);
    out
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
    let mut request = Vec::new();
    let mut buf = vec![0_u8; 64 * 1024];
    let mut needed = None;
    loop {
        if needed.is_none() {
            if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                let length = head
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                needed = Some(end + 4 + length);
            }
        }
        if needed.is_some_and(|needed| request.len() >= needed) {
            return String::from_utf8_lossy(&request).into_owned();
        }
        let read = socket.read(&mut buf).await.unwrap();
        assert!(read > 0, "the client closed the connection early");
        request.extend_from_slice(&buf[..read]);
    }
}

// The server answers one request with each reply, in order. The placeholder `{addr}` in a
// reply header or body is the server address. The handle gives the requests.
pub(crate) async fn serve_script(replies: Vec<Reply>) -> (SocketAddr, JoinHandle<Vec<String>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut requests = Vec::new();
        for reply in replies {
            let (mut socket, _) = listener.accept().await.unwrap();
            requests.push(read_request(&mut socket).await);
            socket.write_all(&reply.into_bytes(addr)).await.unwrap();
        }
        requests
    });
    (addr, server)
}

pub(crate) fn fake_client(addr: SocketAddr) -> GcsClient {
    fake_client_with(addr, user_auth())
}

pub(crate) fn fake_client_with(addr: SocketAddr, auth: Auth) -> GcsClient {
    GcsClient {
        inner: Arc::new(Inner {
            bucket: "my-bucket".to_string(),
            endpoints: Endpoints {
                api_root: format!("http://{addr}/storage/v1"),
                upload_root: format!("http://{addr}/upload/storage/v1"),
                upload_location_prefix: format!("http://{addr}/"),
                iam_root: format!("http://{addr}/iam"),
                metadata_token_url: format!("http://{addr}/token"),
            },
            email: "signer@example.iam.gserviceaccount.com".to_string(),
            auth,
            http: http_client().unwrap(),
            token: tokio::sync::Mutex::new(Some(CachedToken {
                value: "token".to_string(),
                expires_at: Instant::now() + Duration::from_secs(600),
            })),
            ops_log: std::sync::Mutex::new(None),
        }),
    }
}

const META: &str =
    r#"{"name":"a/b.txt","size":"5","contentType":"text/plain","metadata":{"k":"v"}}"#;

fn error_reply(status: u16) -> Reply {
    Reply::new(status, r#"{"error":{"message":"nope"}}"#)
}

fn metadata(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

#[tokio::test]
async fn stat_reads_the_object_and_sends_the_token() {
    let (addr, server) = serve_script(vec![Reply::new(200, META)]).await;
    let meta = fake_client(addr).stat("a/b.txt").await.unwrap();
    assert_eq!(meta.name, "a/b.txt");
    assert_eq!(meta.size, 5);
    assert_eq!(meta.metadata.get("k").map(String::as_str), Some("v"));
    let request = server.await.unwrap().remove(0);
    assert!(
        request.starts_with("GET /storage/v1/b/my-bucket/o/a%2Fb.txt HTTP/1.1"),
        "{request}"
    );
    assert!(request
        .to_ascii_lowercase()
        .contains("authorization: bearer token"));
}

#[tokio::test]
async fn stat_opt_turns_not_found_into_none() {
    let (addr, _server) = serve_script(vec![error_reply(404)]).await;
    assert_eq!(fake_client(addr).stat_opt("a").await.unwrap(), None);
}

#[tokio::test]
async fn an_api_error_keeps_the_status_text_and_the_detail() {
    let (addr, _server) = serve_script(vec![error_reply(403)]).await;
    let err = fake_client(addr).stat("a").await.unwrap_err();
    assert_eq!(err.code(), "GCS_AUTH");
    assert!(err.to_string().contains("status 403"), "{err}");
    assert!(err.to_string().contains("nope"), "{err}");
    let (addr, _server) = serve_script(vec![error_reply(400)]).await;
    let err = fake_client(addr).stat("a").await.unwrap_err();
    assert_eq!(err.code(), "GCS_API");
}

#[tokio::test]
async fn object_calls_reject_a_bad_name_without_a_request() {
    let client = fake_client(([127, 0, 0, 1], 9).into());
    assert!(client.stat("").await.is_err());
    assert!(client.open_object("..").await.is_err());
    assert!(client.delete_object(".").await.is_err());
    assert!(client.rewrite_object("a", "").await.is_err());
    assert!(client.rewrite_object("", "a").await.is_err());
    let bad = metadata(&[("", "v")]);
    assert!(client.patch_metadata("a", &bad).await.is_err());
    let put = PutRequest {
        name: "a".to_string(),
        body: PutBody::Bytes(Vec::new()),
        options: PutOptions {
            content_type: Some(String::new()),
            metadata: BTreeMap::new(),
        },
    };
    assert!(client.put(&put).await.is_err());
    let put = PutRequest {
        options: PutOptions {
            content_type: None,
            metadata: bad,
        },
        ..put
    };
    assert!(client.put(&put).await.is_err());
}

#[test]
fn metadata_with_a_line_break_or_an_empty_key_is_refused() {
    assert!(validate_metadata(&metadata(&[("k", "v")])).is_ok());
    assert!(validate_metadata(&metadata(&[("k", "a\nb")])).is_err());
    assert!(validate_metadata(&metadata(&[("k\r", "v")])).is_err());
    assert!(validate_metadata(&metadata(&[("", "v")])).is_err());
}

#[tokio::test]
async fn get_bytes_reads_the_body_and_opt_turns_not_found_into_none() {
    let (addr, server) = serve_script(vec![Reply::new(200, "hello")]).await;
    let bytes = fake_client(addr).get_bytes("a").await.unwrap();
    assert_eq!(bytes, b"hello");
    let request = server.await.unwrap().remove(0);
    assert!(request.contains("/o/a?alt=media"), "{request}");
    let (addr, _server) = serve_script(vec![error_reply(404), error_reply(404)]).await;
    let client = fake_client(addr);
    assert_eq!(client.get_bytes_opt("a").await.unwrap(), None);
    assert!(client.open_object_opt("a").await.unwrap().is_none());
}

#[tokio::test]
async fn list_sends_the_query_and_reads_the_page() {
    let body = r#"{"items":[{"name":"a","size":"1"}],"prefixes":["p/"],"nextPageToken":"t"}"#;
    let (addr, server) = serve_script(vec![Reply::new(200, body)]).await;
    let query = ListQuery {
        prefix: Some("p/".to_string()),
        ..ListQuery::default()
    };
    let page = fake_client(addr).list(&query).await.unwrap();
    assert_eq!(page.objects.len(), 1);
    assert_eq!(page.prefixes, ["p/"]);
    assert_eq!(page.next_page_token.as_deref(), Some("t"));
    let request = server.await.unwrap().remove(0);
    assert!(
        request.starts_with("GET /storage/v1/b/my-bucket/o?prefix=p%2F HTTP/1.1"),
        "{request}"
    );
}

#[tokio::test]
async fn put_starts_a_session_and_sends_the_bytes() {
    let replies = vec![
        Reply::new(200, "").header("location", "http://{addr}/session?upload_id=x"),
        Reply::new(200, META),
    ];
    let (addr, server) = serve_script(replies).await;
    let request = PutRequest {
        name: "a/b.txt".to_string(),
        body: PutBody::Bytes(b"hello".to_vec()),
        options: PutOptions {
            content_type: Some("text/plain".to_string()),
            metadata: metadata(&[("k", "v")]),
        },
    };
    let meta = fake_client(addr).put(&request).await.unwrap();
    assert_eq!(meta.name, "a/b.txt");
    let requests = server.await.unwrap();
    let start = requests[0].to_ascii_lowercase();
    assert!(start.starts_with("post /upload/storage/v1/b/my-bucket/o?uploadtype=resumable"));
    assert!(start.contains("x-upload-content-length: 5"), "{start}");
    assert!(
        requests[0].contains(r#""metadata":{"k":"v"}"#),
        "{}",
        requests[0]
    );
    assert!(requests[1].starts_with("PUT /session?upload_id=x"));
    assert!(requests[1]
        .to_ascii_lowercase()
        .contains("content-range: bytes 0-4/5"));
    assert!(requests[1].ends_with("hello"));
}

#[tokio::test]
async fn patch_metadata_sends_the_pairs() {
    let (addr, server) = serve_script(vec![Reply::new(200, META)]).await;
    let pairs = metadata(&[("k", "v")]);
    let meta = fake_client(addr)
        .patch_metadata("a/b.txt", &pairs)
        .await
        .unwrap();
    assert_eq!(meta.metadata, pairs);
    let request = server.await.unwrap().remove(0);
    assert!(request.starts_with("PATCH /storage/v1/b/my-bucket/o/a%2Fb.txt"));
    assert!(request.ends_with(r#"{"metadata":{"k":"v"}}"#), "{request}");
}

#[tokio::test]
async fn delete_ignores_a_missing_object_and_reports_other_errors() {
    let replies = vec![Reply::new(204, ""), error_reply(404), error_reply(403)];
    let (addr, server) = serve_script(replies).await;
    let client = fake_client(addr);
    client.delete_object("a").await.unwrap();
    client.delete_object("a").await.unwrap();
    let err = client.delete_object("a").await.unwrap_err();
    assert_eq!(err.code(), "GCS_AUTH");
    let requests = server.await.unwrap();
    assert!(requests[0].starts_with("DELETE /storage/v1/b/my-bucket/o/a "));
}

fn rewrite_next(token: &str) -> Reply {
    Reply::new(
        200,
        &format!(r#"{{"done":false,"rewriteToken":"{token}"}}"#),
    )
}

fn rewrite_done() -> Reply {
    Reply::new(200, &format!(r#"{{"done":true,"resource":{META}}}"#))
}

#[tokio::test]
async fn rewrite_follows_the_token_until_done() {
    let replies = vec![rewrite_next("t1"), rewrite_next("t2"), rewrite_done()];
    let (addr, server) = serve_script(replies).await;
    let meta = fake_client(addr).rewrite_object("a", "b").await.unwrap();
    assert_eq!(meta.name, "a/b.txt");
    let requests = server.await.unwrap();
    assert!(requests[0].contains("/rewriteTo/b/my-bucket/o/b "));
    assert!(requests[1].contains("?rewriteToken=t1 "));
    assert!(requests[2].contains("?rewriteToken=t2 "));
}

#[tokio::test]
async fn rewrite_fails_when_the_token_does_not_change() {
    let (addr, _server) = serve_script(vec![rewrite_next("t1"), rewrite_next("t1")]).await;
    let err = fake_client(addr)
        .rewrite_object("a", "b")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "GCS_API");
    assert!(err.to_string().contains("did not change"), "{err}");
}

#[tokio::test]
async fn move_deletes_the_source_after_the_copy() {
    let (addr, server) = serve_script(vec![rewrite_done(), Reply::new(204, "")]).await;
    let meta = fake_client(addr).move_object("a", "b").await.unwrap();
    assert_eq!(meta.name, "a/b.txt");
    let requests = server.await.unwrap();
    assert!(requests[1].starts_with("DELETE /storage/v1/b/my-bucket/o/a "));
}

#[tokio::test]
async fn move_removes_the_copy_when_the_source_cannot_be_deleted() {
    let replies = vec![
        rewrite_done(),
        error_reply(403),
        Reply::new(200, META),
        Reply::new(204, ""),
    ];
    let (addr, server) = serve_script(replies).await;
    let err = fake_client(addr).move_object("a", "b").await.unwrap_err();
    assert_eq!(err.code(), "GCS_AUTH");
    let requests = server.await.unwrap();
    assert!(requests[3].starts_with("DELETE /storage/v1/b/my-bucket/o/b "));
}

#[tokio::test]
async fn move_reports_both_names_when_the_copy_cannot_be_deleted() {
    let replies = vec![
        rewrite_done(),
        error_reply(403),
        Reply::new(200, META),
        error_reply(403),
    ];
    let (addr, _server) = serve_script(replies).await;
    let err = fake_client(addr).move_object("a", "b").await.unwrap_err();
    assert!(err.to_string().contains("exists at both names"), "{err}");
    let cause = err.source_exception().unwrap();
    assert_eq!(cause.code(), "GCS_AUTH");
    assert!(cause.location().1 > 0);
}

#[tokio::test]
async fn move_keeps_the_status_of_a_retryable_delete_error() {
    let replies = vec![
        rewrite_done(),
        error_reply(503),
        error_reply(503),
        error_reply(503),
        error_reply(503),
        Reply::new(200, META),
        error_reply(403),
    ];
    let (addr, _server) = serve_script(replies).await;
    let err = fake_client(addr).move_object("a", "b").await.unwrap_err();
    assert!(err.to_string().contains("exists at both names"), "{err}");
    assert_eq!(err.code(), "GCS_UNAVAILABLE");
    assert!(crate::gcs::is_retryable(&err));
}

#[tokio::test]
async fn move_reports_the_destination_when_the_source_is_gone() {
    let replies = vec![rewrite_done(), error_reply(403), error_reply(404)];
    let (addr, _server) = serve_script(replies).await;
    let err = fake_client(addr).move_object("a", "b").await.unwrap_err();
    assert!(err.to_string().contains("only at the destination"), "{err}");
}

#[tokio::test]
async fn move_reports_an_unchecked_source() {
    let replies = vec![rewrite_done(), error_reply(403), error_reply(400)];
    let (addr, _server) = serve_script(replies).await;
    let err = fake_client(addr).move_object("a", "b").await.unwrap_err();
    assert!(err.to_string().contains("may exist at both names"), "{err}");
}

#[tokio::test]
async fn send_retries_a_transient_status() {
    let (addr, server) = serve_script(vec![error_reply(503), Reply::new(200, META)]).await;
    let meta = fake_client(addr).stat("a").await.unwrap();
    assert_eq!(meta.size, 5);
    assert_eq!(server.await.unwrap().len(), 2);
}

#[tokio::test]
async fn send_logs_a_failed_response_with_the_bucket() {
    let (addr, _server) = serve_script(vec![error_reply(404)]).await;
    let client = fake_client(addr);
    let config = shipper_config("storage");
    let shipper = LogShipper::new(config);
    client.attach_ops_log(shipper.clone());
    assert!(client.stat("a").await.is_err());
    let events = buffered_events(&shipper);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].extra["status"], "404");
    assert_eq!(events[0].extra["message"], "gcs response");
}

#[tokio::test]
async fn send_accepts_an_extra_status() {
    let (addr, _server) = serve_script(vec![Reply::new(308, "")]).await;
    let response = fake_client(addr)
        .send(&Call {
            method: reqwest::Method::PUT,
            url: format!("http://{addr}/session"),
            headers: vec![(reqwest::header::CONTENT_TYPE, "text/plain".to_string())],
            bytes: Some(b"abc".to_vec()),
            extra_status: &[308],
            timeout: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 308);
}

#[tokio::test]
async fn a_rejected_token_is_dropped_and_fetched_again() {
    let token = r#"{"access_token":"fresh","expires_in":3600}"#;
    let (addr, server) = serve_script(vec![
        error_reply(401),
        Reply::new(200, token),
        Reply::new(200, META),
    ])
    .await;
    let client = fake_client_with(addr, Auth::Metadata);
    let meta = client.stat("a").await.unwrap();
    let requests = server.await.unwrap();
    assert_eq!(meta.name, "a/b.txt");
    assert!(requests[1].starts_with("GET /token?scopes="));
    assert!(requests[2]
        .to_lowercase()
        .contains("authorization: bearer fresh"));
}

#[tokio::test]
async fn metadata_auth_fetches_a_token_from_the_metadata_server() {
    let body = r#"{"access_token":"abc","expires_in":3600}"#;
    let (addr, server) = serve_script(vec![Reply::new(200, body)]).await;
    let http = http_client().unwrap();
    let token = Auth::Metadata
        .fetch_token(&http, &format!("http://{addr}/token"))
        .await
        .unwrap();
    server.abort();
    assert_eq!(token.value, "abc");
}

#[tokio::test]
async fn client_rejects_an_empty_bucket() {
    let err = GcsClient::new(" ", user_auth()).await.unwrap_err();
    assert_eq!(err.code(), "GCS_CONFIG");
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
    assert_eq!(err.code(), "GCS_CONFIG");
}

#[test]
fn not_found_becomes_none() {
    let missing: Result<(), Exception> = Err(exception!("missing", code = "GCS_NOT_FOUND"));
    assert!(not_found_as_none(missing).unwrap().is_none());
    assert_eq!(not_found_as_none(Ok(7)).unwrap(), Some(7));
    let denied: Result<(), Exception> = Err(exception!("denied", code = "GCS_AUTH"));
    assert_eq!(not_found_as_none(denied).unwrap_err().code(), "GCS_AUTH");
}

#[tokio::test]
async fn move_rejects_the_same_object() {
    let client = GcsClient::new("my-bucket", user_auth()).await.unwrap();
    let err = client.move_object("a.txt", "a.txt").await.unwrap_err();
    assert_eq!(err.code(), "GCS_CONFIG");
}

#[tokio::test]
async fn signed_url_rejects_a_lifetime_over_seven_days() {
    let client = GcsClient::new("my-bucket", user_auth()).await.unwrap();
    let err = client
        .signed_get_url("a", Some(Duration::from_secs(604_801)), None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "GCS_CONFIG");
}

#[tokio::test]
async fn signed_url_rejects_a_missing_or_zero_lifetime() {
    let client = GcsClient::new("my-bucket", user_auth()).await.unwrap();
    for lifetime in [None, Some(Duration::ZERO), Some(Duration::from_millis(500))] {
        let err = client
            .signed_get_url("a", lifetime, None)
            .await
            .unwrap_err();
        assert_eq!(err.code(), "GCS_CONFIG");
    }
}

#[tokio::test]
async fn ops_log_writes_the_bucket_and_omits_the_query() {
    let client = GcsClient::new("my-bucket", user_auth()).await.unwrap();
    let config = shipper_config("storage");
    let shipper = LogShipper::new(config);
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
    crate::test::logging::assert_uuid_v7(&events[0].id);
    assert!(events[0].context.is_empty());
    assert_eq!(events[0].extra["status"], "200");
    assert_eq!(events[0].extra["message"], "gcs response");
    assert_eq!(events[0].latency_ms, Some(3));
    assert_eq!(events[0].extra.get("bucket"), Some(&"my-bucket".into()));
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
    assert_eq!(err.code(), "GCS_TRANSPORT");
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
