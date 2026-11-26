mod log_buffer;

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ian_framework_rs::exception::Exception;
use ian_framework_rs::logging::logger::log_buffer::{CAPACITY, CAPACITY_WARN_NOTE, SHIP_CHUNK};
use ian_framework_rs::logging::{HttpLogShipper, LogEvent, LogRequest, LogShipper, Logger};
use ian_framework_rs::mutex;
use ian_framework_rs::shutdown::ShutdownReceiver;

use crate::logging::{
    buffered_events, default_mask_patterns, default_shipper, global_logger, http_shipper, logger,
};

const LIMIT: usize = CAPACITY / 5 * 4;

#[test]
fn init_keeps_the_first_logger() {
    let first = global_logger();
    let second = logger(default_shipper("my-app"));
    let err = Logger::init(second).unwrap_err();
    assert_eq!(err.code(), "LOGGER_ALREADY_SET");
    assert!(std::ptr::eq(Logger::get().unwrap(), first));
}

struct MemoryShipper {
    shipped: Arc<Mutex<Vec<LogEvent>>>,
}

impl LogShipper for MemoryShipper {
    fn ship<'a>(
        &'a self,
        events: &'a [LogEvent],
    ) -> Pin<Box<dyn Future<Output = Result<(), Exception>> + Send + 'a>> {
        Box::pin(async move {
            mutex::lock(&self.shipped).extend_from_slice(events);
            Ok(())
        })
    }
}

#[tokio::test]
async fn a_project_shipper_gets_the_masked_batch() {
    let shipped = Arc::new(Mutex::new(Vec::new()));
    let shipper = Logger::new(
        MemoryShipper {
            shipped: shipped.clone(),
        },
        default_mask_patterns(),
    )
    .unwrap();
    shipper.record(LogEvent::info("one").with_extra("password", "hunter2"));
    shipper.record(LogEvent::info("two"));
    assert_eq!(shipper.send_buffer().await.unwrap(), 2);
    assert_eq!(shipper.buffered_len(), 0);
    let shipped = mutex::lock(&shipped);
    assert_eq!(shipped.len(), 2);
    assert_eq!(shipped[0].action, "one");
    assert_eq!(shipped[0].extra["password"], "***");
    assert_eq!(shipped[1].action, "two");
}

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

#[tokio::test]
async fn empty_buffer_does_not_post() {
    let shipper = logger(http_shipper("api", "http://127.0.0.1:9"));
    assert_eq!(shipper.send_buffer().await.unwrap(), 0);
    assert_eq!(shipper.buffered_len(), 0);
}

#[tokio::test]
async fn unreachable_endpoint_keeps_the_buffer() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let shipper = logger(http_shipper("api", &format!("http://{addr}")));
    shipper.record(LogEvent::error("down", "boom"));
    let err = shipper.send_buffer().await.unwrap_err();
    assert_eq!(err.code(), "LOG_SHIP_FAILED");
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
    let _ = shipper.send_buffer().await;
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
    let _ = shipper.send_buffer().await;
    assert_eq!(shipper.buffered_len(), 0);
}

#[tokio::test]
async fn send_buffer_sends_the_post_api_key_as_a_bearer_token() {
    let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
    let shipper = logger(HttpLogShipper::new("api", endpoint, "log-post-key").unwrap());
    shipper.record(LogEvent::info("one"));
    let _ = shipper.send_buffer().await;
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
    assert_eq!(shipper.send_buffer().await.unwrap(), SHIP_CHUNK);
    assert_eq!(shipper.buffered_len(), 1);
    let events = parse_body(&rx.recv().await.unwrap().body);
    assert_eq!(events.len(), SHIP_CHUNK);
    assert_eq!(events[0].action, "0");
    assert_eq!(events[SHIP_CHUNK - 1].action, (SHIP_CHUNK - 1).to_string());
    assert_eq!(buffered_events(&shipper)[0].action, SHIP_CHUNK.to_string());
    let _ = shipper.send_buffer().await;
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
    let _ = shipper.send_buffer().await;
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
    assert_eq!(ship.await.unwrap().unwrap(), 2);
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
async fn shutdown_flush_stops_after_three_failed_retries() {
    let (endpoint, mut rx) = capture_server(StatusAction::Unavailable).await;
    let shipper = logger(http_shipper("api", &endpoint));
    shipper.record(LogEvent::info("keep"));
    let shutdown = ShutdownReceiver::new();
    shutdown.trigger();
    let traced = Arc::new(Mutex::new(Vec::new()));
    let _guard = tracing::subscriber::set_default(FlushReport {
        events: traced.clone(),
    });
    let handle = shipper.spawn_ship_loop(shutdown);
    tokio::time::timeout(Duration::from_secs(5), handle)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(buffered_events(&shipper)[0].action, "keep");
    let reports = mutex::lock(&traced);
    assert!(reports.iter().any(|report| {
        report.message == "cannot flush buffered log events before shutdown"
            && report.buffered == Some(1)
    }));
    drop(reports);
    let mut attempts = 0;
    while rx.try_recv().is_ok() {
        attempts += 1;
    }
    assert_eq!(attempts, 3);
}

#[tokio::test]
async fn shutdown_flush_retries_a_failed_batch_until_it_leaves() {
    let (endpoint, mut rx) = capture_server(StatusAction::UnavailableThenOk(2)).await;
    let shipper = logger(http_shipper("api", &endpoint));
    shipper.record(LogEvent::info("keep"));
    let shutdown = ShutdownReceiver::new();
    shutdown.trigger();
    let handle = shipper.spawn_ship_loop(shutdown);
    tokio::time::timeout(Duration::from_secs(5), handle)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(shipper.buffered_len(), 0);
    let mut attempts = 0;
    while rx.try_recv().is_ok() {
        attempts += 1;
    }
    assert_eq!(attempts, 3);
}

#[tokio::test]
async fn failed_ship_does_not_repeat_the_capacity_warning() {
    let (endpoint, _rx) = capture_server(StatusAction::Unavailable).await;
    let shipper = logger(http_shipper("api", &endpoint));
    fill(&shipper, LIMIT);
    assert_eq!(warn_count(&buffered_events(&shipper)), 1);
    let _ = shipper.send_buffer().await;
    shipper.record(LogEvent::info("more"));
    assert_eq!(warn_count(&buffered_events(&shipper)), 1);
}

#[tokio::test]
async fn successful_ship_below_80_percent_warns_on_the_next_crossing() {
    let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
    let shipper = logger(http_shipper("api", &endpoint));
    fill(&shipper, LIMIT);
    let _ = shipper.send_buffer().await;
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
    let _ = shipper.send_buffer().await;
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
    UnavailableThenOk(u8),
}

#[tokio::test]
async fn send_all_posts_every_chunk() {
    let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
    let shipper = logger(http_shipper("api", &format!("{endpoint}/")));
    fill(&shipper, SHIP_CHUNK * 2 + 1);
    shipper.send_all().await;
    assert_eq!(shipper.buffered_len(), 0);
    for count in [SHIP_CHUNK, SHIP_CHUNK, 1] {
        let captured = rx.recv().await.unwrap();
        assert_eq!(captured.path, "/log/api");
        assert_eq!(parse_body(&captured.body).len(), count);
    }
}

#[tokio::test]
async fn send_all_stops_at_a_failed_chunk() {
    let (endpoint, mut rx) = capture_server(StatusAction::Unavailable).await;
    let shipper = logger(http_shipper("api", &endpoint));
    fill(&shipper, SHIP_CHUNK + 1);
    shipper.send_all().await;
    assert_eq!(shipper.buffered_len(), SHIP_CHUNK + 1);
    assert!(rx.recv().await.is_some());
    assert!(rx.try_recv().is_err());
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
    use axum;

    #[derive(Clone)]
    struct Capture {
        tx: tokio::sync::mpsc::UnboundedSender<Captured>,
        action: u8,
        fails_left: std::sync::Arc<std::sync::atomic::AtomicU8>,
    }

    let (action_code, fails_left) = match action {
        StatusAction::NoContent => (0, 0),
        StatusAction::Unavailable => (1, 0),
        StatusAction::Slow => (2, 0),
        StatusAction::UnavailableThenOk(fails) => (3, fails),
    };
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let app = axum::Router::new()
        .route(
            "/log/{app}",
            axum::routing::post(
                |axum::extract::State(capture): axum::extract::State<Capture>,
                 axum::extract::Path(app): axum::extract::Path<String>,
                 uri: axum::http::Uri,
                 headers: axum::http::HeaderMap,
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
                    let fail = capture.action == 1
                        || (capture.action == 3
                            && capture
                                .fails_left
                                .try_update(
                                    std::sync::atomic::Ordering::Relaxed,
                                    std::sync::atomic::Ordering::Relaxed,
                                    |left| left.checked_sub(1),
                                )
                                .is_ok());
                    if fail {
                        axum::http::StatusCode::SERVICE_UNAVAILABLE
                    } else {
                        axum::http::StatusCode::NO_CONTENT
                    }
                },
            ),
        )
        .with_state(Capture {
            tx,
            action: action_code,
            fails_left: std::sync::Arc::new(std::sync::atomic::AtomicU8::new(fails_left)),
        });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), rx)
}

struct FlushReport {
    events: Arc<Mutex<Vec<TracedFlush>>>,
}

struct TracedFlush {
    buffered: Option<u64>,
    message: String,
}

impl tracing::Subscriber for FlushReport {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        *metadata.level() == tracing::Level::ERROR
    }

    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }

    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        let mut traced = TracedFlush {
            buffered: None,
            message: String::new(),
        };
        event.record(&mut traced);
        mutex::lock(&self.events).push(traced);
    }

    fn enter(&self, _: &tracing::span::Id) {}

    fn exit(&self, _: &tracing::span::Id) {}
}

impl tracing::field::Visit for TracedFlush {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "message" => self.message = format!("{value:?}").trim_matches('"').to_string(),
            "buffered" => self.buffered = format!("{value:?}").parse().ok(),
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if field.name() == "buffered" {
            self.buffered = Some(value);
        }
    }
}
