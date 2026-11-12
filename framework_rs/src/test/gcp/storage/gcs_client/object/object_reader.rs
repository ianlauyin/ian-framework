use futures_util::StreamExt;

use crate::test::gcp::storage::gcs_client::{fake_client, serve_script, Reply};

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
    assert_eq!(err.code(), "GCS_CONFIG");
}

#[tokio::test]
async fn read_to_vec_reports_a_body_that_ends_early() {
    let (addr, _server) = serve_script(vec![Reply::new(200, "abc").declared_length(10)]).await;
    let reader = fake_client(addr).open_object("a").await.unwrap();
    let err = reader.read_to_vec().await.unwrap_err();
    assert_eq!(err.code(), "GCS_TRANSPORT");
}

#[tokio::test]
async fn byte_stream_yields_the_chunks_and_then_an_error() {
    let (addr, _server) = serve_script(vec![Reply::new(200, "abc").declared_length(10)]).await;
    let reader = fake_client(addr).open_object("a").await.unwrap();
    let items: Vec<_> = reader.byte_stream().collect().await;
    assert_eq!(items[0].as_ref().unwrap(), b"abc");
    let err = items.last().unwrap().as_ref().unwrap_err();
    assert_eq!(err.code(), "GCS_TRANSPORT");
}
