mod client_log;
mod event;
mod log_shipper;
mod logger;
mod with_log;

use std::sync::{Mutex, Once};

use axum::http::{header, StatusCode};
use axum::routing::post;
use axum::{Json, Router};

use crate::logging::*;

pub(crate) const REFUSED_ACTION: &str = "refused";

static GLOBAL_LOGGER: Once = Once::new();
static SHIPPED: Mutex<Vec<Vec<LogEvent>>> = Mutex::new(Vec::new());

pub(crate) fn http_shipper(app: &'static str, endpoint: &str) -> HttpLogShipper {
    HttpLogShipper::new(app, endpoint.to_string(), "APP_KEY").unwrap()
}

pub(crate) fn default_shipper(app: &'static str) -> HttpLogShipper {
    http_shipper(app, "https://logs.example")
}

pub(crate) fn default_mask_patterns() -> Vec<String> {
    DEFAULT_MASK_PATTERNS
        .iter()
        .map(|pattern| pattern.to_string())
        .collect()
}

pub(crate) fn logger(shipper: HttpLogShipper) -> Logger {
    Logger::new(shipper, default_mask_patterns()).unwrap()
}

pub(crate) fn client_log_event(event: &LogEvent) -> ClientLogEvent {
    let mut wire = serde_json::to_value(event).unwrap();
    let fields = wire.as_object_mut().unwrap();
    for (snake, camel) in [
        ("latency_ms", "latencyMs"),
        ("error_message", "errorMessage"),
    ] {
        if let Some(value) = fields.remove(snake) {
            fields.insert(camel.to_string(), value);
        }
    }
    serde_json::from_value(wire).unwrap()
}

pub(crate) fn assert_uuid_v7(id: &str) {
    let uuid = uuid::Uuid::parse_str(id).unwrap();
    assert_eq!(uuid.get_version_num(), 7);
}

pub(crate) fn buffered_events(logger: &Logger) -> Vec<LogEvent> {
    logger.lock_buffer().events.iter().cloned().collect()
}

// All tests share the one global logger, so a test finds its own events by a unique action or URL.
// The log server refuses a batch that holds the action `REFUSED_ACTION`.
pub(crate) fn global_logger() -> &'static Logger {
    GLOBAL_LOGGER.call_once(|| {
        let endpoint = start_log_server();
        let _ = Logger::init(logger(http_shipper("my-app", &endpoint)));
    });
    Logger::get().unwrap()
}

// `Logger::direct_send` ships in a spawned task, so a test waits for the result.
pub(crate) async fn wait_for(done: impl Fn() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("the condition did not become true in 2 seconds");
}

pub(crate) fn shipped_batches(action: &str) -> Vec<Vec<LogEvent>> {
    SHIPPED
        .lock()
        .unwrap()
        .iter()
        .filter(|batch| batch.iter().any(|event| event.action == action))
        .cloned()
        .collect()
}

// The server runs on its own thread and runtime, because the runtime of one test ends with that test.
fn start_log_server() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            let app = Router::new().route("/log/{app}", post(receive_batch));
            axum::serve(listener, app).await.unwrap();
        });
    });
    endpoint
}

// `connection: close` stops the shipper from reusing a connection of a runtime that has ended.
async fn receive_batch(
    Json(log): Json<LogRequest>,
) -> (StatusCode, [(header::HeaderName, &'static str); 1]) {
    let refused = log
        .events
        .iter()
        .any(|event| event.action == REFUSED_ACTION);
    SHIPPED.lock().unwrap().push(log.events);
    let status = if refused {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::NO_CONTENT
    };
    (status, [(header::CONNECTION, "close")])
}
