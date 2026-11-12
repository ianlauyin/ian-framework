use chrono::{DateTime, Utc};
use futures_util::StreamExt;

use crate::gcp::storage::object::*;
use crate::test::gcp::storage::fake_client;
use crate::test::gcp::{serve_script, Reply};

#[test]
fn object_json_keeps_caller_metadata() {
    let json = br#"{
        "name": "photos/cat.jpeg",
        "size": "2048",
        "contentType": "image/jpeg",
        "updated": "2020-01-02T03:04:05.678Z",
        "metadata": {"owner": "alice", "owner-id": "7"}
    }"#;
    let meta = ObjectMeta::from_slice(json).unwrap();
    assert_eq!(meta.name, "photos/cat.jpeg");
    assert_eq!(meta.size, 2048);
    assert_eq!(meta.content_type.as_deref(), Some("image/jpeg"));
    assert_eq!(
        meta.updated,
        Some(
            DateTime::parse_from_rfc3339("2020-01-02T03:04:05.678Z")
                .unwrap()
                .with_timezone(&Utc)
        )
    );
    assert_eq!(
        meta.metadata.get("owner").map(String::as_str),
        Some("alice")
    );
    assert_eq!(meta.metadata.get("owner-id").map(String::as_str), Some("7"));
}

#[tokio::test]
async fn read_to_vec_returns_the_whole_body() {
    let (addr, _server) = serve_script(vec![Reply::new(200, "hello")]).await;
    let reader = fake_client(addr).open_object("a").await.unwrap();
    assert_eq!(reader.read_to_vec().await.unwrap(), b"hello");
}

#[tokio::test]
async fn read_to_vec_refuses_a_declared_size_over_the_limit() {
    let declared = 300 * 1024 * 1024;
    let (addr, _server) = serve_script(vec![Reply::new(200, "").declared_length(declared)]).await;
    let reader = fake_client(addr).open_object("a").await.unwrap();
    let err = reader.read_to_vec().await.unwrap_err();
    assert_eq!(err.code(), "GCP_CONFIG");
}

#[tokio::test]
async fn read_to_vec_reports_a_body_that_ends_early() {
    let (addr, _server) = serve_script(vec![Reply::new(200, "abc").declared_length(10)]).await;
    let reader = fake_client(addr).open_object("a").await.unwrap();
    let err = reader.read_to_vec().await.unwrap_err();
    assert_eq!(err.code(), "GCP_TRANSPORT");
}

#[tokio::test]
async fn byte_stream_yields_the_chunks_and_then_an_error() {
    let (addr, _server) = serve_script(vec![Reply::new(200, "abc").declared_length(10)]).await;
    let reader = fake_client(addr).open_object("a").await.unwrap();
    let items: Vec<_> = reader.byte_stream().collect().await;
    assert_eq!(&items[0].as_ref().unwrap()[..], b"abc");
    let err = items.last().unwrap().as_ref().unwrap_err();
    assert_eq!(err.code(), "GCP_TRANSPORT");
}

#[tokio::test]
async fn reader_debug_shows_the_status_and_the_length() {
    let (addr, _server) = serve_script(vec![Reply::new(200, "hello")]).await;
    let reader = fake_client(addr).open_object("a").await.unwrap();
    let text = format!("{reader:?}");
    assert!(text.contains("status: 200"), "{text}");
    assert!(text.contains("content_length: Some(5)"), "{text}");
}
