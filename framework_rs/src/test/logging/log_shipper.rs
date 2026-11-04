use std::time::Duration;

use crate::logging::{LogEvent, LogLevel};
use crate::shutdown::ShutdownReceiver;

use crate::logging::log_buffer::{CAPACITY, CAPACITY_WARN_NOTE};
use crate::logging::log_shipper::*;
use crate::logging::ShipperConfig;
use crate::test::logging::{buffered_events, shipper_config};

const LIMIT: usize = CAPACITY / 5 * 4;

fn config(app: &str, endpoint: impl Into<String>) -> ShipperConfig {
    let mut config = shipper_config(app);
    config.endpoint = endpoint.into();
    config
}

fn buffered() -> LogShipper {
    LogShipper::new(config("api", "https://logs.example"))
}

fn fill(shipper: &LogShipper, count: usize) {
    for index in 0..count {
        shipper.record(LogEvent::info(index.to_string()));
    }
}

fn warn_count(events: &[LogEvent]) -> usize {
    events
        .iter()
        .filter(|event| event.action == CAPACITY_WARN_NOTE)
        .count()
}

fn parse_body(body: &str) -> Vec<LogEvent> {
    serde_json::from_str(body).unwrap()
}

#[test]
fn defaults() {
    let shipper = buffered();
    assert_eq!(shipper.inner.app, "api");
    assert_eq!(SHIP_CHUNK, 1_000);
    assert_eq!(SHIP_TIMEOUT, Duration::from_secs(10));
    assert_eq!(CAPACITY, 20_000);
    assert_eq!(SHIP_EVERY, Duration::from_secs(60));
}

#[test]
#[should_panic(expected = "is not a valid regex")]
fn invalid_mask_pattern_panics() {
    let mut config = config("api", "https://logs.example");
    config.mask_patterns = vec!["(".to_string()];
    LogShipper::new(config);
}

#[test]
#[should_panic(expected = "not a valid header value")]
fn invalid_post_api_key_panics() {
    let mut config = config("api", "https://logs.example");
    config.post_api_key = "log\u{7f}key".to_string();
    LogShipper::new(config);
}

#[test]
fn debug_hides_the_post_api_key() {
    let mut config = config("api", "https://logs.example");
    config.post_api_key = "log-post-key".to_string();
    let debug = format!("{:?}", LogShipper::new(config));
    assert!(!debug.contains("log-post-key"));
    assert!(debug.contains("post_api_key: \"***\""));
}

#[test]
fn record_keeps_the_event_and_omits_scope_keys() {
    let shipper = buffered();
    let mut event = LogEvent::info(format!("GET /{}", "p".repeat(3000)));
    event.id = "i".repeat(200);
    event
        .context
        .insert("user_agent".to_string(), "é".repeat(2000).into());
    let stored_copy = event.clone();
    shipper.record(event);
    let stored = buffered_events(&shipper).remove(0);
    assert_eq!(stored.action, stored_copy.action);
    assert_eq!(stored.id, stored_copy.id);
    assert_eq!(stored.context, stored_copy.context);
    let value: serde_json::Value = serde_json::to_value(&stored).unwrap();
    assert!(value.get("app").is_none());
    assert!(value.get("project").is_none());
}

#[test]
fn record_masks_the_value_of_a_matching_key() {
    let mut config = config("api", "https://logs.example");
    config.mask_patterns = vec![r"(?i)^(password|token)$".to_string()];
    let shipper = LogShipper::new(config);
    let mut event = LogEvent::info("login");
    event.extra.insert("password".to_string(), "hunter2".into());
    event.extra.insert("TOKEN".to_string(), 1234u64.into());
    event.extra.insert("user".to_string(), "alice".into());
    shipper.record(event);
    let stored = buffered_events(&shipper).remove(0);
    assert_eq!(stored.extra["password"], serde_json::json!("***"));
    assert_eq!(stored.extra["TOKEN"], serde_json::json!("***"));
    assert_eq!(stored.extra["user"], serde_json::json!("alice"));
}

#[test]
fn record_masks_every_pattern_match_in_string_values() {
    let mut config = config("api", "https://logs.example");
    config.mask_patterns = vec![
        r"password=\S+".to_string(),
        r"(?i)bearer [A-Za-z0-9._-]+".to_string(),
    ];
    let shipper = LogShipper::new(config);
    let mut event = LogEvent::info("POST /login?password=x");
    event.context.insert(
        "user_agent".to_string(),
        "agent BEARER abc.def".to_string().into(),
    );
    event.extra.insert(
        "note".to_string(),
        "user password=a, BEARER b.c done".into(),
    );
    shipper.record(event);
    let stored = buffered_events(&shipper).remove(0);
    assert_eq!(stored.action, "POST /login?password=x");
    assert_eq!(stored.context["user_agent"], "agent ***");
    assert_eq!(stored.extra["note"], serde_json::json!("user *** *** done"));
}

#[test]
fn default_patterns_mask_the_value_of_a_name_value_pair() {
    let shipper = buffered();
    let mut event = LogEvent::info("GET /");
    event.context.insert(
        "request_url".to_string(),
        "/x?a=1&access_token=abc123&page=2&Password=hunter2"
            .to_string()
            .into(),
    );
    shipper.record(event);
    let stored = buffered_events(&shipper).remove(0);
    assert_eq!(stored.context["request_url"], "/x?a=1&***&page=2&***");
}

#[test]
fn record_without_mask_patterns_keeps_the_text() {
    let mut config = config("api", "https://logs.example");
    config.mask_patterns = Vec::new();
    let shipper = LogShipper::new(config);
    let mut event = LogEvent::info("GET /");
    event
        .extra
        .insert("note".to_string(), "password=hunter2".into());
    shipper.record(event);
    let stored = buffered_events(&shipper).remove(0);
    assert_eq!(stored.extra["note"], serde_json::json!("password=hunter2"));
}

#[test]
fn record_drops_the_oldest_event_when_full() {
    let shipper = buffered();
    fill(&shipper, CAPACITY + 5);
    let events = buffered_events(&shipper);
    assert_eq!(events.len(), CAPACITY);
    assert_ne!(events[0].action, "0");
}

#[test]
fn record_inserts_one_capacity_warning_at_80_percent() {
    let shipper = buffered();
    fill(&shipper, LIMIT - 1);
    let below = buffered_events(&shipper);
    assert_eq!(below.len(), LIMIT - 1);
    assert_eq!(warn_count(&below), 0);
    let before = chrono::Utc::now();
    shipper.record(LogEvent::info("crossing"));
    let after = chrono::Utc::now();
    let events = buffered_events(&shipper);
    assert_eq!(events.len(), LIMIT + 1);
    assert_eq!(events[LIMIT - 1].action, "crossing");
    let warn = &events[LIMIT];
    assert_eq!(warn.action, CAPACITY_WARN_NOTE);
    assert_eq!(warn.level, LogLevel::Warn);
    assert_eq!(warn.id, "");
    assert!(warn.context.is_empty());
    assert!(warn.extra.is_empty());
    assert!(warn.ts >= before && warn.ts <= after);
    shipper.record(LogEvent::info("next"));
    assert_eq!(warn_count(&buffered_events(&shipper)), 1);
}

#[test]
fn record_drops_the_oldest_event_to_store_the_capacity_warning() {
    let shipper = buffered();
    fill(&shipper, CAPACITY);
    let events = buffered_events(&shipper);
    assert_eq!(events.len(), CAPACITY);
    assert_eq!(warn_count(&events), 1);
}

#[tokio::test]
async fn empty_buffer_does_not_post() {
    let shipper = LogShipper::new(config("api", "http://127.0.0.1:9"));
    shipper.send_buffer().await;
    assert_eq!(shipper.buffered_len(), 0);
}

#[tokio::test]
async fn unreachable_endpoint_keeps_the_buffer() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let shipper = LogShipper::new(config("api", format!("http://{addr}")));
    shipper.record(LogEvent::error("down", "boom"));
    shipper.send_buffer().await;
    assert_eq!(shipper.buffered_len(), 1);
    assert_eq!(buffered_events(&shipper)[0].action, "down");
}

#[tokio::test]
async fn send_buffer_posts_json_to_the_app_path() {
    let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
    let shipper = LogShipper::new(config("my-app", endpoint));
    shipper.record(LogEvent::info("one"));
    shipper.record(LogEvent::warn("two"));
    shipper.record(LogEvent::error("three", "boom"));
    shipper.send_buffer().await;
    assert_eq!(shipper.buffered_len(), 0);
    let captured = rx.recv().await.unwrap();
    assert_eq!(captured.app, "my-app");
    assert_eq!(captured.path, "/log/my-app");
    assert_eq!(captured.content_type, "application/json");
    let events = parse_body(&captured.body);
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].action, "one");
    assert_eq!(events[1].action, "two");
    assert_eq!(events[2].action, "three");
    let body: serde_json::Value = serde_json::from_str(&captured.body).unwrap();
    for item in body.as_array().unwrap() {
        assert!(item.get("app").is_none());
        assert!(item.get("project").is_none());
        assert!(item.get("action").is_some());
    }
    shipper.send_buffer().await;
    assert_eq!(shipper.buffered_len(), 0);
}

#[tokio::test]
async fn send_buffer_sends_the_post_api_key_as_a_bearer_token() {
    let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
    let mut config = config("api", endpoint);
    config.post_api_key = "log-post-key".to_string();
    let shipper = LogShipper::new(config);
    shipper.record(LogEvent::info("one"));
    shipper.send_buffer().await;
    let captured = rx.recv().await.unwrap();
    assert_eq!(
        captured.authorization.as_deref(),
        Some("Bearer log-post-key")
    );
    assert_eq!(shipper.buffered_len(), 0);
}

#[tokio::test]
async fn send_buffer_posts_one_chunk_and_leaves_the_rest() {
    let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
    let shipper = LogShipper::new(config("api", endpoint));
    fill(&shipper, SHIP_CHUNK + 1);
    shipper.send_buffer().await;
    assert_eq!(shipper.buffered_len(), 1);
    let events = parse_body(&rx.recv().await.unwrap().body);
    assert_eq!(events.len(), SHIP_CHUNK);
    assert_eq!(events[0].action, "0");
    assert_eq!(events[SHIP_CHUNK - 1].action, (SHIP_CHUNK - 1).to_string());
    assert_eq!(buffered_events(&shipper)[0].action, SHIP_CHUNK.to_string());
    shipper.send_buffer().await;
    assert_eq!(shipper.buffered_len(), 0);
    let rest = parse_body(&rx.recv().await.unwrap().body);
    assert_eq!(rest.len(), 1);
    assert_eq!(rest[0].action, SHIP_CHUNK.to_string());
}

#[tokio::test]
async fn http_error_restores_the_batch() {
    let (endpoint, _rx) = capture_server(StatusAction::Unavailable).await;
    let shipper = LogShipper::new(config("api", endpoint));
    shipper.record(LogEvent::info("keep"));
    shipper.send_buffer().await;
    assert_eq!(buffered_events(&shipper)[0].action, "keep");
}

#[tokio::test]
async fn in_flight_records_stay_out_of_a_successful_batch() {
    let (endpoint, _rx) = capture_server(StatusAction::Slow).await;
    let shipper = LogShipper::new(config("api", endpoint));
    shipper.record(LogEvent::info("a"));
    shipper.record(LogEvent::info("b"));
    let shipper_task = shipper.clone();
    let ship = tokio::spawn(async move { shipper_task.send_buffer().await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while shipper.buffered_len() != 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    shipper.record(LogEvent::info("c"));
    ship.await.unwrap();
    let left = buffered_events(&shipper);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].action, "c");
}

#[tokio::test]
async fn shutdown_flushes_every_chunk_and_ends_the_loop() {
    let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
    let shipper = LogShipper::new(config("api", endpoint));
    let shutdown = ShutdownReceiver::new();
    let handle = shipper.spawn_ship_loop(shutdown.clone());
    tokio::time::sleep(Duration::from_millis(20)).await;
    fill(&shipper, SHIP_CHUNK + 1);
    shutdown.trigger();
    tokio::time::timeout(Duration::from_secs(2), handle)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(shipper.buffered_len(), 0);
    assert_eq!(parse_body(&rx.recv().await.unwrap().body).len(), SHIP_CHUNK);
    assert_eq!(parse_body(&rx.recv().await.unwrap().body).len(), 1);
}

#[tokio::test]
async fn shutdown_flush_stops_when_the_endpoint_fails() {
    let (endpoint, _rx) = capture_server(StatusAction::Unavailable).await;
    let shipper = LogShipper::new(config("api", endpoint));
    shipper.record(LogEvent::info("keep"));
    let shutdown = ShutdownReceiver::new();
    shutdown.trigger();
    let handle = shipper.spawn_ship_loop(shutdown);
    tokio::time::timeout(Duration::from_secs(2), handle)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(buffered_events(&shipper)[0].action, "keep");
}

#[tokio::test]
async fn failed_ship_does_not_repeat_the_capacity_warning() {
    let (endpoint, _rx) = capture_server(StatusAction::Unavailable).await;
    let shipper = LogShipper::new(config("api", endpoint));
    fill(&shipper, LIMIT);
    assert_eq!(warn_count(&buffered_events(&shipper)), 1);
    shipper.send_buffer().await;
    shipper.record(LogEvent::info("more"));
    assert_eq!(warn_count(&buffered_events(&shipper)), 1);
}

#[tokio::test]
async fn successful_ship_below_80_percent_warns_on_the_next_crossing() {
    let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
    let shipper = LogShipper::new(config("api", endpoint));
    fill(&shipper, LIMIT);
    shipper.send_buffer().await;
    let posted = parse_body(&rx.recv().await.unwrap().body);
    assert_eq!(warn_count(&posted), 0);
    assert_eq!(shipper.buffered_len(), LIMIT + 1 - SHIP_CHUNK);
    fill(&shipper, SHIP_CHUNK - 1);
    assert_eq!(warn_count(&buffered_events(&shipper)), 2);
}

#[tokio::test]
async fn successful_ship_at_80_percent_does_not_warn_again() {
    let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
    let shipper = LogShipper::new(config("api", endpoint));
    fill(&shipper, CAPACITY);
    shipper.send_buffer().await;
    let posted = parse_body(&rx.recv().await.unwrap().body);
    assert_eq!(warn_count(&posted), 0);
    assert!(shipper.buffered_len() >= LIMIT);
    shipper.record(LogEvent::info("more"));
    assert_eq!(warn_count(&buffered_events(&shipper)), 1);
}

enum StatusAction {
    NoContent,
    Unavailable,
    Slow,
}

struct Captured {
    app: String,
    path: String,
    content_type: String,
    authorization: Option<String>,
    body: String,
}

async fn capture_server(
    action: StatusAction,
) -> (String, tokio::sync::mpsc::UnboundedReceiver<Captured>) {
    use axum::extract::{Path, State};
    use axum::http::{HeaderMap, StatusCode, Uri};
    use axum::routing::post;
    use axum::Router;

    #[derive(Clone)]
    struct Capture {
        tx: tokio::sync::mpsc::UnboundedSender<Captured>,
        action: u8,
    }

    let action_code = match action {
        StatusAction::NoContent => 0,
        StatusAction::Unavailable => 1,
        StatusAction::Slow => 2,
    };
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let app = Router::new()
        .route(
            "/log/{app}",
            post(
                |State(capture): State<Capture>,
                 Path(app): Path<String>,
                 uri: Uri,
                 headers: HeaderMap,
                 body: String| async move {
                    if capture.action == 2 {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                    let content_type = headers
                        .get(axum::http::header::CONTENT_TYPE)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("")
                        .to_string();
                    let authorization = headers
                        .get(axum::http::header::AUTHORIZATION)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_string);
                    let _ = capture.tx.send(Captured {
                        app,
                        path: uri.path().to_string(),
                        content_type,
                        authorization,
                        body,
                    });
                    if capture.action == 1 {
                        StatusCode::SERVICE_UNAVAILABLE
                    } else {
                        StatusCode::NO_CONTENT
                    }
                },
            ),
        )
        .with_state(Capture {
            tx,
            action: action_code,
        });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), rx)
}
