use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::event::level_rank;
use super::{to_jsonl, truncate, LogError, LogErrorKind, LogEvent, LogLevel};

pub const DEFAULT_CAPACITY: usize = 10_000;
pub const DEFAULT_BATCH_SIZE: usize = 100;
pub const DEFAULT_SHIP_EVERY: Duration = Duration::from_secs(5);
pub const SHIP_TIMEOUT: Duration = Duration::from_secs(10);
pub const MAX_MESSAGE_BYTES: usize = 4096;
pub const MAX_REQUEST_BYTES: usize = 2080;
pub const MAX_ID_BYTES: usize = 128;
pub const MAX_IP_BYTES: usize = 64;
pub const MAX_USER_AGENT_BYTES: usize = 1024;

const MAX_SCOPE_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipperConfig {
    pub app: String,
    pub project: String,
    pub endpoint: Option<String>,
    pub capacity: usize,
    pub batch_size: usize,
    pub minimum_level: LogLevel,
}

pub fn shipper_config(app: impl Into<String>) -> ShipperConfig {
    ShipperConfig {
        app: app.into(),
        project: String::new(),
        endpoint: None,
        capacity: DEFAULT_CAPACITY,
        batch_size: DEFAULT_BATCH_SIZE,
        minimum_level: LogLevel::Info,
    }
}

#[derive(Clone)]
pub struct LogShipper {
    inner: Arc<Inner>,
}

struct Inner {
    app: String,
    project: String,
    endpoint: Option<String>,
    capacity: usize,
    batch_size: usize,
    minimum_level: LogLevel,
    buffer: Mutex<VecDeque<LogEvent>>,
    dropped: AtomicU64,
    http: Option<reqwest::Client>,
    shipping: tokio::sync::Mutex<()>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShipMode {
    BufferOnly,
    Sent,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipResult {
    pub shipped: usize,
    pub buffered: usize,
    pub mode: ShipMode,
}

impl std::fmt::Debug for LogShipper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogShipper")
            .field("app", &self.inner.app)
            .field("project", &self.inner.project)
            .field("endpoint", &self.inner.endpoint)
            .field("buffered", &buffered_len(self))
            .finish_non_exhaustive()
    }
}

pub fn log_shipper(config: ShipperConfig) -> Result<LogShipper, LogError> {
    require_scope_name(&config.app, "app")?;
    if !config.project.is_empty() {
        require_scope_name(&config.project, "project")?;
    }
    if config.capacity == 0 {
        return Err(LogError::new(LogErrorKind::Config, "log capacity is zero"));
    }
    if config.batch_size == 0 {
        return Err(LogError::new(
            LogErrorKind::Config,
            "log batch size is zero",
        ));
    }
    if config.batch_size > config.capacity {
        return Err(LogError::new(
            LogErrorKind::Config,
            "log batch size is above the capacity",
        ));
    }
    let endpoint = normalize_endpoint(config.endpoint)?;
    let http = match &endpoint {
        Some(_) => Some(http_client()?),
        None => None,
    };
    Ok(LogShipper {
        inner: Arc::new(Inner {
            app: config.app,
            project: config.project,
            endpoint,
            capacity: config.capacity,
            batch_size: config.batch_size,
            minimum_level: config.minimum_level,
            buffer: Mutex::new(VecDeque::new()),
            dropped: AtomicU64::new(0),
            http,
            shipping: tokio::sync::Mutex::new(()),
        }),
    })
}

pub fn app(shipper: &LogShipper) -> &str {
    &shipper.inner.app
}

pub fn project(shipper: &LogShipper) -> &str {
    &shipper.inner.project
}

pub fn endpoint(shipper: &LogShipper) -> Option<&str> {
    shipper.inner.endpoint.as_deref()
}

pub fn minimum_level(shipper: &LogShipper) -> LogLevel {
    shipper.inner.minimum_level
}

pub fn buffered_len(shipper: &LogShipper) -> usize {
    lock_buffer(&shipper.inner.buffer).len()
}

pub fn buffered_events(shipper: &LogShipper) -> Vec<LogEvent> {
    lock_buffer(&shipper.inner.buffer).iter().cloned().collect()
}

pub fn dropped(shipper: &LogShipper) -> u64 {
    shipper.inner.dropped.load(Ordering::Relaxed)
}

pub fn record(shipper: &LogShipper, mut event: LogEvent) -> Result<(), LogError> {
    if !event.app.is_empty() && event.app != shipper.inner.app {
        return Err(LogError::new(
            LogErrorKind::Config,
            "event app does not match the shipper",
        ));
    }
    if level_rank(event.level) < level_rank(shipper.inner.minimum_level) {
        return Ok(());
    }
    event.app = shipper.inner.app.clone();
    if event.project.is_empty() {
        event.project = shipper.inner.project.clone();
    }
    event.message = truncate(&event.message, MAX_MESSAGE_BYTES).to_string();
    event.request = truncate(&event.request, MAX_REQUEST_BYTES).to_string();
    event.id = truncate(&event.id, MAX_ID_BYTES).to_string();
    event.ip = truncate(&event.ip, MAX_IP_BYTES).to_string();
    event.user_agent = truncate(&event.user_agent, MAX_USER_AGENT_BYTES).to_string();
    push_event(&shipper.inner, event);
    Ok(())
}

pub async fn ship_batch(shipper: &LogShipper) -> Result<ShipResult, LogError> {
    let _shipping = shipper.inner.shipping.lock().await;
    let Some(endpoint) = shipper.inner.endpoint.clone() else {
        return Ok(ShipResult {
            shipped: 0,
            buffered: buffered_len(shipper),
            mode: ShipMode::BufferOnly,
        });
    };
    let batch = take_batch(&shipper.inner);
    if batch.is_empty() {
        return Ok(ShipResult {
            shipped: 0,
            buffered: 0,
            mode: ShipMode::Empty,
        });
    }
    let Some(client) = shipper.inner.http.clone() else {
        restore_batch(&shipper.inner, batch);
        return Err(LogError::new(
            LogErrorKind::Config,
            "shipper has an endpoint and no HTTP client",
        ));
    };
    if let Err(err) = post_batch(&client, &endpoint, &shipper.inner.app, &batch).await {
        restore_batch(&shipper.inner, batch);
        return Err(err);
    }
    Ok(ShipResult {
        shipped: batch.len(),
        buffered: buffered_len(shipper),
        mode: ShipMode::Sent,
    })
}

pub fn spawn_ship_loop(
    shipper: LogShipper,
    every: Duration,
) -> Result<tokio::task::JoinHandle<()>, LogError> {
    if every.is_zero() {
        return Err(LogError::new(LogErrorKind::Config, "ship interval is zero"));
    }
    Ok(tokio::spawn(async move {
        let mut ticker = tokio::time::interval(every);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            let _ = ship_batch(&shipper).await;
        }
    }))
}

fn push_event(inner: &Inner, event: LogEvent) {
    let mut buffer = lock_buffer(&inner.buffer);
    while buffer.len() >= inner.capacity {
        buffer.pop_front();
        inner.dropped.fetch_add(1, Ordering::Relaxed);
    }
    buffer.push_back(event);
}

fn take_batch(inner: &Inner) -> Vec<LogEvent> {
    let mut buffer = lock_buffer(&inner.buffer);
    let count = inner.batch_size.min(buffer.len());
    buffer.drain(..count).collect()
}

fn restore_batch(inner: &Inner, batch: Vec<LogEvent>) {
    let mut buffer = lock_buffer(&inner.buffer);
    for event in batch.into_iter().rev() {
        buffer.push_front(event);
    }
    while buffer.len() > inner.capacity {
        buffer.pop_back();
        inner.dropped.fetch_add(1, Ordering::Relaxed);
    }
}

fn lock_buffer(
    buffer: &Mutex<VecDeque<LogEvent>>,
) -> std::sync::MutexGuard<'_, VecDeque<LogEvent>> {
    buffer.lock().unwrap_or_else(|err| err.into_inner())
}

async fn post_batch(
    client: &reqwest::Client,
    endpoint: &str,
    app: &str,
    events: &[LogEvent],
) -> Result<(), LogError> {
    let body = to_jsonl(events)?;
    let url = batch_url(endpoint, app);
    let response = client
        .post(&url)
        .timeout(SHIP_TIMEOUT)
        .header(reqwest::header::CONTENT_TYPE, "application/x-ndjson")
        .body(body)
        .send()
        .await
        .map_err(|err| {
            LogError::with_source(LogErrorKind::Transport, "log ship request failed", err)
        })?;
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    let text = response.text().await.unwrap_or_default();
    Err(LogError::with_status(
        LogErrorKind::Api,
        status.as_u16(),
        format!(
            "log server status {}: {}",
            status.as_u16(),
            truncate(&text, 300)
        ),
    ))
}

fn batch_url(endpoint: &str, app: &str) -> String {
    format!("{endpoint}/log/{app}")
}

fn require_scope_name(value: &str, label: &str) -> Result<(), LogError> {
    if valid_scope_name(value) {
        return Ok(());
    }
    Err(LogError::new(
        LogErrorKind::Config,
        format!("{label} name is empty or contains a forbidden character"),
    ))
}

fn valid_scope_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() || value.len() > MAX_SCOPE_LEN {
        return false;
    }
    value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
}

fn normalize_endpoint(endpoint: Option<String>) -> Result<Option<String>, LogError> {
    let Some(raw) = endpoint else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().any(char::is_whitespace) {
        return Err(LogError::new(
            LogErrorKind::Config,
            "log endpoint contains whitespace",
        ));
    }
    let parsed = reqwest::Url::parse(trimmed)
        .map_err(|_| LogError::new(LogErrorKind::Config, "log endpoint is not an http URL"))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(LogError::new(
            LogErrorKind::Config,
            "log endpoint scheme must be http or https",
        ));
    }
    if parsed.host_str().is_none() {
        return Err(LogError::new(
            LogErrorKind::Config,
            "log endpoint has no host",
        ));
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(LogError::new(
            LogErrorKind::Config,
            "log endpoint must not contain a query or a fragment",
        ));
    }
    Ok(Some(parsed.to_string().trim_end_matches('/').to_string()))
}

fn http_client() -> Result<reqwest::Client, LogError> {
    reqwest::Client::builder()
        .connect_timeout(SHIP_TIMEOUT)
        .timeout(SHIP_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("ian_framework/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|err| {
            LogError::with_source(LogErrorKind::Transport, "cannot build the HTTP client", err)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::event;

    fn buffer_only() -> LogShipper {
        log_shipper(shipper_config("api")).unwrap()
    }

    fn labeled() -> LogShipper {
        let mut config = shipper_config("api");
        config.project = "demo".to_string();
        log_shipper(config).unwrap()
    }

    #[test]
    fn defaults_are_buffer_only() {
        let shipper = buffer_only();
        assert_eq!(app(&shipper), "api");
        assert_eq!(project(&shipper), "");
        assert_eq!(endpoint(&shipper), None);
        assert_eq!(minimum_level(&shipper), LogLevel::Info);
        assert_eq!(DEFAULT_CAPACITY, 10_000);
        assert_eq!(DEFAULT_BATCH_SIZE, 100);
        assert_eq!(DEFAULT_SHIP_EVERY, Duration::from_secs(5));
        assert_eq!(SHIP_TIMEOUT, Duration::from_secs(10));
    }

    #[test]
    fn blank_endpoint_is_buffer_only() {
        for endpoint in [Some(String::new()), Some("   ".to_string())] {
            let mut config = shipper_config("api");
            config.endpoint = endpoint;
            let shipper = log_shipper(config).unwrap();
            assert_eq!(super::endpoint(&shipper), None);
        }
    }

    #[test]
    fn endpoint_is_normalized() {
        let mut config = shipper_config("cs");
        config.endpoint = Some("http://logs.example/ingest/".to_string());
        let shipper = log_shipper(config).unwrap();
        assert_eq!(endpoint(&shipper), Some("http://logs.example/ingest"));
        assert_eq!(
            batch_url(endpoint(&shipper).unwrap(), app(&shipper)),
            "http://logs.example/ingest/log/cs"
        );
    }

    #[test]
    fn invalid_config_is_rejected() {
        let cases = [
            shipper_config(""),
            shipper_config("a/b"),
            {
                let mut config = shipper_config("api");
                config.project = "a/b".to_string();
                config
            },
            {
                let mut config = shipper_config("api");
                config.capacity = 0;
                config
            },
            {
                let mut config = shipper_config("api");
                config.batch_size = 0;
                config
            },
            {
                let mut config = shipper_config("api");
                config.capacity = 2;
                config.batch_size = 3;
                config
            },
            {
                let mut config = shipper_config("api");
                config.endpoint = Some("ftp://logs.example".to_string());
                config
            },
            {
                let mut config = shipper_config("api");
                config.endpoint = Some("http://logs.example/a b".to_string());
                config
            },
        ];
        for config in cases {
            assert_eq!(
                log_shipper(config).unwrap_err().kind(),
                LogErrorKind::Config
            );
        }
    }

    #[test]
    fn record_fills_app_and_drops_debug_below_info() {
        let shipper = labeled();
        record(&shipper, event("", LogLevel::Debug, "hidden")).unwrap();
        assert_eq!(buffered_len(&shipper), 0);
        record(&shipper, event("", LogLevel::Info, "kept")).unwrap();
        let events = buffered_events(&shipper);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].app, "api");
        assert_eq!(events[0].project, "demo");
        assert_eq!(events[0].message, "kept");
    }

    #[test]
    fn record_keeps_a_different_project() {
        let shipper = labeled();
        let mut event = event("api", LogLevel::Info, "kept");
        event.project = "other".to_string();
        record(&shipper, event).unwrap();
        let stored = buffered_events(&shipper).remove(0);
        assert_eq!(stored.project, "other");
        assert_eq!(stored.app, "api");
    }

    #[test]
    fn record_rejects_a_different_app() {
        let shipper = buffer_only();
        let err = record(&shipper, event("other", LogLevel::Debug, "no")).unwrap_err();
        assert_eq!(err.kind(), LogErrorKind::Config);
        assert_eq!(buffered_len(&shipper), 0);
    }

    #[test]
    fn capacity_drops_the_oldest_event() {
        let mut config = shipper_config("api");
        config.capacity = 2;
        config.batch_size = 1;
        let shipper = log_shipper(config).unwrap();
        record(&shipper, event("api", LogLevel::Info, "a")).unwrap();
        record(&shipper, event("api", LogLevel::Info, "b")).unwrap();
        record(&shipper, event("api", LogLevel::Info, "c")).unwrap();
        let messages: Vec<_> = buffered_events(&shipper)
            .into_iter()
            .map(|event| event.message)
            .collect();
        assert_eq!(messages, ["b", "c"]);
        assert_eq!(dropped(&shipper), 1);
    }

    #[test]
    fn record_truncates_id_request_ip_and_user_agent() {
        let shipper = buffer_only();
        let mut event = event("api", LogLevel::Info, "ok");
        event.id = "i".repeat(MAX_ID_BYTES + 8);
        event.request = format!("GET /{}", "p".repeat(MAX_REQUEST_BYTES));
        event.ip = "1".repeat(MAX_IP_BYTES + 8);
        event.user_agent = "é".repeat(MAX_USER_AGENT_BYTES);
        record(&shipper, event).unwrap();
        let stored = buffered_events(&shipper).remove(0);
        assert_eq!(stored.id.len(), MAX_ID_BYTES);
        assert!(stored.request.len() <= MAX_REQUEST_BYTES);
        assert!(stored.request.len() > MAX_REQUEST_BYTES - 4);
        assert!(stored.request.is_char_boundary(stored.request.len()));
        assert_eq!(stored.ip.len(), MAX_IP_BYTES);
        assert!(stored.user_agent.len() <= MAX_USER_AGENT_BYTES);
        assert!(stored.user_agent.is_char_boundary(stored.user_agent.len()));
        assert_eq!(MAX_ID_BYTES, 128);
        assert_eq!(MAX_REQUEST_BYTES, 2080);
        assert_eq!(MAX_IP_BYTES, 64);
        assert_eq!(MAX_USER_AGENT_BYTES, 1024);
    }

    #[test]
    fn message_is_truncated_on_a_char_boundary() {
        let shipper = buffer_only();
        let message = "é".repeat(3000);
        record(&shipper, event("api", LogLevel::Info, message)).unwrap();
        let stored = buffered_events(&shipper).remove(0).message;
        assert!(stored.len() <= MAX_MESSAGE_BYTES);
        assert!(stored.is_char_boundary(stored.len()));
        assert!(stored.chars().next().is_some());
    }

    #[tokio::test]
    async fn unset_endpoint_keeps_events_without_http() {
        let shipper = buffer_only();
        record(&shipper, event("api", LogLevel::Info, "local")).unwrap();
        let result = ship_batch(&shipper).await.unwrap();
        assert_eq!(result.mode, ShipMode::BufferOnly);
        assert_eq!(result.shipped, 0);
        assert_eq!(result.buffered, 1);
        assert_eq!(buffered_len(&shipper), 1);
        assert_eq!(buffered_events(&shipper)[0].message, "local");
    }

    #[tokio::test]
    async fn empty_batch_does_not_post() {
        let mut config = shipper_config("api");
        config.endpoint = Some("http://127.0.0.1:9".to_string());
        let shipper = log_shipper(config).unwrap();
        let result = ship_batch(&shipper).await.unwrap();
        assert_eq!(result.mode, ShipMode::Empty);
        assert_eq!(result.shipped, 0);
    }

    #[tokio::test]
    async fn unreachable_log_server_keeps_the_buffer() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let mut config = shipper_config("api");
        config.endpoint = Some(format!("http://{addr}"));
        let shipper = log_shipper(config).unwrap();
        record(&shipper, event("api", LogLevel::Error, "down")).unwrap();
        let err = ship_batch(&shipper).await.unwrap_err();
        assert_eq!(err.kind(), LogErrorKind::Transport);
        assert_eq!(buffered_len(&shipper), 1);
        assert_eq!(buffered_events(&shipper)[0].message, "down");
    }

    #[tokio::test]
    async fn ship_batch_posts_ndjson_to_the_app_path() {
        let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
        let mut config = shipper_config("cs");
        config.project = "demo".to_string();
        config.endpoint = Some(endpoint);
        config.batch_size = 2;
        config.capacity = 10;
        let shipper = log_shipper(config).unwrap();
        let mut one = event("cs", LogLevel::Info, "one");
        one.project = "other".to_string();
        record(&shipper, one).unwrap();
        record(&shipper, event("cs", LogLevel::Warn, "two")).unwrap();
        record(&shipper, event("cs", LogLevel::Error, "three")).unwrap();
        let first = ship_batch(&shipper).await.unwrap();
        assert_eq!(first.mode, ShipMode::Sent);
        assert_eq!(first.shipped, 2);
        assert_eq!(first.buffered, 1);
        let captured = rx.recv().await.unwrap();
        assert_eq!(captured.app, "cs");
        assert_eq!(captured.path, "/log/cs");
        assert_eq!(captured.content_type, "application/x-ndjson");
        assert!(!captured.has_project_header);
        let events = parse_body(&captured.body);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].message, "one");
        assert_eq!(events[0].app, "cs");
        assert_eq!(events[0].project, "other");
        assert_eq!(events[1].message, "two");
        assert_eq!(events[1].app, "cs");
        assert_eq!(events[1].project, "demo");
        let second = ship_batch(&shipper).await.unwrap();
        assert_eq!(second.shipped, 1);
        assert_eq!(buffered_len(&shipper), 0);
        let third = ship_batch(&shipper).await.unwrap();
        assert_eq!(third.mode, ShipMode::Empty);
    }

    #[tokio::test]
    async fn http_error_restores_the_batch() {
        let (endpoint, _rx) = capture_server(StatusAction::Unavailable).await;
        let mut config = shipper_config("api");
        config.endpoint = Some(endpoint);
        let shipper = log_shipper(config).unwrap();
        record(&shipper, event("api", LogLevel::Info, "keep")).unwrap();
        let err = ship_batch(&shipper).await.unwrap_err();
        assert_eq!(err.kind(), LogErrorKind::Api);
        assert!(err.to_string().contains("503"));
        assert_eq!(buffered_events(&shipper)[0].message, "keep");
    }

    #[tokio::test]
    async fn in_flight_records_stay_out_of_a_successful_batch() {
        let (endpoint, _rx) = capture_server(StatusAction::Slow).await;
        let mut config = shipper_config("api");
        config.endpoint = Some(endpoint);
        config.capacity = 2;
        config.batch_size = 2;
        let shipper = log_shipper(config).unwrap();
        record(&shipper, event("api", LogLevel::Info, "a")).unwrap();
        record(&shipper, event("api", LogLevel::Info, "b")).unwrap();
        let shipper_task = shipper.clone();
        let ship = tokio::spawn(async move { ship_batch(&shipper_task).await.unwrap() });
        tokio::time::timeout(Duration::from_secs(2), async {
            while buffered_len(&shipper) != 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        record(&shipper, event("api", LogLevel::Info, "c")).unwrap();
        let result = ship.await.unwrap();
        assert_eq!(result.shipped, 2);
        assert_eq!(result.mode, ShipMode::Sent);
        let left = buffered_events(&shipper);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].message, "c");
    }

    #[tokio::test]
    async fn failed_restore_keeps_the_oldest_batch() {
        let (endpoint, _rx) = capture_server(StatusAction::SlowUnavailable).await;
        let mut config = shipper_config("api");
        config.endpoint = Some(endpoint);
        config.capacity = 2;
        config.batch_size = 2;
        let shipper = log_shipper(config).unwrap();
        record(&shipper, event("api", LogLevel::Info, "a")).unwrap();
        record(&shipper, event("api", LogLevel::Info, "b")).unwrap();
        let shipper_task = shipper.clone();
        let ship = tokio::spawn(async move { ship_batch(&shipper_task).await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while buffered_len(&shipper) != 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        record(&shipper, event("api", LogLevel::Info, "c")).unwrap();
        let err = ship.await.unwrap().unwrap_err();
        assert_eq!(err.kind(), LogErrorKind::Api);
        let left: Vec<_> = buffered_events(&shipper)
            .into_iter()
            .map(|event| event.message)
            .collect();
        assert_eq!(left, ["a", "b"]);
        assert_eq!(dropped(&shipper), 1);
    }

    #[tokio::test]
    async fn buffer_only_loop_does_not_drop_events() {
        let shipper = buffer_only();
        record(&shipper, event("api", LogLevel::Info, "stay")).unwrap();
        let handle = spawn_ship_loop(shipper.clone(), Duration::from_millis(10)).unwrap();
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(!handle.is_finished());
        handle.abort();
        assert_eq!(buffered_len(&shipper), 1);
        assert_eq!(
            spawn_ship_loop(shipper, Duration::ZERO).unwrap_err().kind(),
            LogErrorKind::Config
        );
    }

    fn parse_body(body: &str) -> Vec<LogEvent> {
        crate::logging::parse_jsonl(body).unwrap()
    }

    enum StatusAction {
        NoContent,
        Unavailable,
        Slow,
        SlowUnavailable,
    }

    struct Captured {
        app: String,
        path: String,
        content_type: String,
        has_project_header: bool,
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
            StatusAction::SlowUnavailable => 3,
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
                        if capture.action == 2 || capture.action == 3 {
                            tokio::time::sleep(Duration::from_millis(200)).await;
                        }
                        let content_type = headers
                            .get(axum::http::header::CONTENT_TYPE)
                            .and_then(|value| value.to_str().ok())
                            .unwrap_or("")
                            .to_string();
                        let _ = capture.tx.send(Captured {
                            app,
                            path: uri.path().to_string(),
                            content_type,
                            has_project_header: headers.get("x-project").is_some(),
                            body,
                        });
                        if capture.action == 1 || capture.action == 3 {
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
}
