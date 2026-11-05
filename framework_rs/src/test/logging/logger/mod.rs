use std::collections::BTreeMap;
use std::time::Duration;

use crate::logging::log_shipper::SHIP_CHUNK;
use crate::logging::logger::log_buffer::{CAPACITY, CAPACITY_WARN_NOTE};
use crate::logging::{HttpLogShipper, LogEvent, LogLevel, LogRequest, Logger};
use crate::shutdown::ShutdownReceiver;
use crate::test::logging::{buffered_events, default_shipper, http_shipper, logger};

#[test]
fn logs_without_a_shipper_and_keeps_the_first_shipper() {
    Logger::error("TEST/error", BTreeMap::new(), "failed".into());
    if Logger::get().is_none() {
        let first = logger(default_shipper("my-app"));
        assert!(Logger::init(first).is_ok());
    }
    let second = logger(default_shipper("my-app"));
    assert!(Logger::init(second).is_err());
    Logger::warn("TEST/warn", BTreeMap::new());
    Logger::warn_with("TEST/warn", BTreeMap::new(), "slow".into());
}

#[test]
fn error_adds_the_exception_trace_to_the_message() {
    let messages = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let capture = Capture(std::sync::Arc::clone(&messages));
    let root = crate::exception!(code = "ENV_INVALID", "port is not a number");
    let top = crate::exception!(code = "ENV_MISSING", "app cannot start", source = root);
    tracing::subscriber::with_default(capture, || {
        Logger::error("TEST/exceptionTrace", BTreeMap::new(), Box::new(top));
        Logger::error("TEST/plain", BTreeMap::new(), "slow".into());
    });
    let messages = messages.lock().unwrap();
    let text = messages[0].as_str();
    assert!(text.starts_with("app cannot start\n0: ERROR [ENV_MISSING] app cannot start at "));
    assert!(text.contains("\n1: ERROR [ENV_INVALID] port is not a number at "));
    assert_eq!(messages[1], "slow");
}

struct Capture(std::sync::Arc<std::sync::Mutex<Vec<String>>>);

struct MessageVisitor<'a>(&'a mut Option<String>);

impl tracing::field::Visit for MessageVisitor<'_> {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "error_message" {
            *self.0 = Some(value.to_string());
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "error_message" && self.0.is_none() {
            let text = format!("{value:?}");
            let text = text
                .strip_prefix("Some(\"")
                .and_then(|text| text.strip_suffix("\")"))
                .unwrap_or(&text);
            *self.0 = Some(text.replace("\\n", "\n").replace("\\\"", "\""));
        }
    }
}

impl tracing::Subscriber for Capture {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        metadata.is_event()
    }

    fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }

    fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        let mut message = None;
        event.record(&mut MessageVisitor(&mut message));
        if let Some(message) = message {
            self.0.lock().unwrap().push(message);
        }
    }

    fn enter(&self, _span: &tracing::span::Id) {}

    fn exit(&self, _span: &tracing::span::Id) {}
}

const LIMIT: usize = CAPACITY / 5 * 4;

fn buffered() -> Logger {
    logger(http_shipper("api", "https://logs.example"))
}

fn fill(shipper: &Logger, count: usize) {
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
    serde_json::from_str::<LogRequest>(body).unwrap().events
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
#[should_panic(expected = "is not a valid regex")]
fn invalid_mask_pattern_panics() {
    Logger::new(default_shipper("api"), vec!["(".to_string()]);
}

#[test]
fn record_masks_the_value_of_a_matching_key() {
    let shipper = Logger::new(
        default_shipper("api"),
        vec![r"(?i)^(password|token)$".to_string()],
    );
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
    let shipper = Logger::new(
        default_shipper("api"),
        vec![
            r"password=\S+".to_string(),
            r"(?i)bearer [A-Za-z0-9._-]+".to_string(),
        ],
    );
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
    let shipper = Logger::new(default_shipper("api"), Vec::new());
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
    crate::test::logging::assert_uuid_v7(&warn.id);
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
    let shipper = logger(http_shipper("api", "http://127.0.0.1:9"));
    shipper.send_buffer().await;
    assert_eq!(shipper.buffered_len(), 0);
}

#[tokio::test]
async fn unreachable_endpoint_keeps_the_buffer() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let shipper = logger(http_shipper("api", &format!("http://{addr}")));
    shipper.record(LogEvent::error("down", "boom"));
    shipper.send_buffer().await;
    assert_eq!(shipper.buffered_len(), 1);
    assert_eq!(buffered_events(&shipper)[0].action, "down");
}

#[tokio::test]
async fn send_buffer_posts_json_to_the_app_path() {
    let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
    let shipper = logger(http_shipper("my-app", &endpoint));
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
    assert_eq!(body.as_object().unwrap().len(), 1);
    for item in body["events"].as_array().unwrap() {
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
    let shipper = logger(HttpLogShipper::new("api", endpoint, "log-post-key"));
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
    let shipper = logger(http_shipper("api", &endpoint));
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
    let shipper = logger(http_shipper("api", &endpoint));
    shipper.record(LogEvent::info("keep"));
    shipper.send_buffer().await;
    assert_eq!(buffered_events(&shipper)[0].action, "keep");
}

#[tokio::test]
async fn in_flight_records_stay_out_of_a_successful_batch() {
    let (endpoint, _rx) = capture_server(StatusAction::Slow).await;
    let shipper = logger(http_shipper("api", &endpoint));
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
    let shipper = logger(http_shipper("api", &endpoint));
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
    let shipper = logger(http_shipper("api", &endpoint));
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
    let shipper = logger(http_shipper("api", &endpoint));
    fill(&shipper, LIMIT);
    assert_eq!(warn_count(&buffered_events(&shipper)), 1);
    shipper.send_buffer().await;
    shipper.record(LogEvent::info("more"));
    assert_eq!(warn_count(&buffered_events(&shipper)), 1);
}

#[tokio::test]
async fn successful_ship_below_80_percent_warns_on_the_next_crossing() {
    let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
    let shipper = logger(http_shipper("api", &endpoint));
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
    let shipper = logger(http_shipper("api", &endpoint));
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
