use std::time::Duration;

use axum::body::to_bytes;
use axum::response::IntoResponse;
use serde::Serialize;
use tokio::sync::broadcast;

use ian_framework_rs::http::sse::*;
use ian_framework_rs::shutdown::ShutdownReceiver;

const CHANGED: ChangedView = ChangedView { r#type: "changed" };

#[derive(Clone, Serialize)]
struct ChangedView {
    r#type: &'static str,
}

async fn read_body(
    receiver: broadcast::Receiver<ChangedView>,
    shutdown: ShutdownReceiver,
) -> String {
    let response = broadcast_sse(receiver, shutdown, Duration::from_secs(60)).into_response();
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    let body = tokio::time::timeout(
        Duration::from_secs(1),
        to_bytes(response.into_body(), usize::MAX),
    )
    .await
    .unwrap()
    .unwrap();
    String::from_utf8(body.to_vec()).unwrap()
}

#[tokio::test]
async fn sends_each_message_as_json_and_ends_when_the_sender_closes() {
    let (sender, receiver) = broadcast::channel(4);
    assert!(sender.send(CHANGED).is_ok());
    assert!(sender.send(CHANGED).is_ok());
    drop(sender);
    let body = read_body(receiver, ShutdownReceiver::new()).await;
    assert_eq!(body, "data: {\"type\":\"changed\"}\n\n".repeat(2));
}

#[tokio::test]
async fn skips_lost_messages_after_a_lag() {
    let (sender, receiver) = broadcast::channel(1);
    assert!(sender.send(CHANGED).is_ok());
    assert!(sender.send(CHANGED).is_ok());
    drop(sender);
    let body = read_body(receiver, ShutdownReceiver::new()).await;
    assert_eq!(body, "data: {\"type\":\"changed\"}\n\n");
}

#[tokio::test]
async fn ends_at_shutdown() {
    let (sender, receiver) = broadcast::channel::<ChangedView>(4);
    let shutdown = ShutdownReceiver::new();
    shutdown.trigger();
    let body = read_body(receiver, shutdown).await;
    assert!(body.is_empty());
    drop(sender);
}
