use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;

use super::event::level_rank;
use super::{truncate, LogError, LogErrorKind, LogEvent, LogLevel};

const DEFAULT_CAPACITY: usize = 10_000;
const DEFAULT_BATCH_SIZE: usize = 100;
#[allow(dead_code)]
const DEFAULT_SHIP_EVERY: Duration = Duration::from_secs(5);
const SHIP_TIMEOUT: Duration = Duration::from_secs(10);

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

impl ShipperConfig {
    pub fn new(app: impl Into<String>) -> Self {
        Self {
            app: app.into(),
            project: String::new(),
            endpoint: None,
            capacity: DEFAULT_CAPACITY,
            batch_size: DEFAULT_BATCH_SIZE,
            minimum_level: LogLevel::Info,
        }
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
            .field("buffered", &self.buffered_len())
            .finish_non_exhaustive()
    }
}

impl LogShipper {
    pub fn new(config: ShipperConfig) -> Result<Self, LogError> {
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
        Ok(Self {
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

    pub fn app(&self) -> &str {
        &self.inner.app
    }

    pub fn project(&self) -> &str {
        &self.inner.project
    }

    pub fn endpoint(&self) -> Option<&str> {
        self.inner.endpoint.as_deref()
    }

    pub fn minimum_level(&self) -> LogLevel {
        self.inner.minimum_level
    }

    pub fn buffered_len(&self) -> usize {
        lock_buffer(&self.inner.buffer).len()
    }

    pub fn buffered_events(&self) -> Vec<LogEvent> {
        lock_buffer(&self.inner.buffer).iter().cloned().collect()
    }

    pub fn dropped(&self) -> u64 {
        self.inner.dropped.load(Ordering::Relaxed)
    }

    pub fn record(&self, event: LogEvent) -> Result<(), LogError> {
        if level_rank(event.level) < level_rank(self.inner.minimum_level) {
            return Ok(());
        }
        push_event(&self.inner, event);
        Ok(())
    }

    pub async fn ship_batch(&self) -> Result<ShipResult, LogError> {
        let _shipping = self.inner.shipping.lock().await;
        let Some(endpoint) = self.inner.endpoint.clone() else {
            return Ok(ShipResult {
                shipped: 0,
                buffered: self.buffered_len(),
                mode: ShipMode::BufferOnly,
            });
        };
        let batch = take_batch(&self.inner);
        if batch.is_empty() {
            return Ok(ShipResult {
                shipped: 0,
                buffered: 0,
                mode: ShipMode::Empty,
            });
        }
        let Some(client) = self.inner.http.clone() else {
            restore_batch(&self.inner, batch);
            return Err(LogError::new(
                LogErrorKind::Config,
                "shipper has an endpoint and no HTTP client",
            ));
        };
        if let Err(err) = post_batch(&client, &endpoint, &self.inner.app, &batch).await {
            restore_batch(&self.inner, batch);
            return Err(err);
        }
        Ok(ShipResult {
            shipped: batch.len(),
            buffered: self.buffered_len(),
            mode: ShipMode::Sent,
        })
    }

    pub fn spawn_ship_loop(
        &self,
        every: Duration,
    ) -> Result<tokio::task::JoinHandle<()>, LogError> {
        if every.is_zero() {
            return Err(LogError::new(LogErrorKind::Config, "ship interval is zero"));
        }
        let shipper = self.clone();
        Ok(tokio::spawn(async move {
            let mut ticker = tokio::time::interval(every);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                let _ = shipper.ship_batch().await;
            }
        }))
    }
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

#[derive(Serialize)]
struct RequestJson<'a> {
    events: &'a [LogEvent],
}

async fn post_batch(
    client: &reqwest::Client,
    endpoint: &str,
    app: &str,
    events: &[LogEvent],
) -> Result<(), LogError> {
    let body = serde_json::to_string(&RequestJson { events }).map_err(|err| {
        LogError::with_source(LogErrorKind::Encode, "cannot encode a log batch", err)
    })?;
    let url = batch_url(endpoint, app);
    let response = client
        .post(&url)
        .timeout(SHIP_TIMEOUT)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
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
        LogShipper::new(ShipperConfig::new("api")).unwrap()
    }

    fn labeled() -> LogShipper {
        let mut config = ShipperConfig::new("api");
        config.project = "demo".to_string();
        LogShipper::new(config).unwrap()
    }

    #[test]
    fn defaults_are_buffer_only() {
        let shipper = buffer_only();
        assert_eq!(shipper.app(), "api");
        assert_eq!(shipper.project(), "");
        assert_eq!(shipper.endpoint(), None);
        assert_eq!(shipper.minimum_level(), LogLevel::Info);
        assert_eq!(DEFAULT_CAPACITY, 10_000);
        assert_eq!(DEFAULT_BATCH_SIZE, 100);
        assert_eq!(DEFAULT_SHIP_EVERY, Duration::from_secs(5));
        assert_eq!(SHIP_TIMEOUT, Duration::from_secs(10));
    }

    #[test]
    fn blank_endpoint_is_buffer_only() {
        for endpoint in [Some(String::new()), Some("   ".to_string())] {
            let mut config = ShipperConfig::new("api");
            config.endpoint = endpoint;
            let shipper = LogShipper::new(config).unwrap();
            assert_eq!(shipper.endpoint(), None);
        }
    }

    #[test]
    fn endpoint_is_normalized() {
        let mut config = ShipperConfig::new("cs");
        config.endpoint = Some("http://logs.example/ingest/".to_string());
        let shipper = LogShipper::new(config).unwrap();
        assert_eq!(shipper.endpoint(), Some("http://logs.example/ingest"));
        assert_eq!(
            batch_url(shipper.endpoint().unwrap(), shipper.app()),
            "http://logs.example/ingest/log/cs"
        );
    }

    #[test]
    fn invalid_config_is_rejected() {
        let cases = [
            ShipperConfig::new(""),
            ShipperConfig::new("a/b"),
            {
                let mut config = ShipperConfig::new("api");
                config.project = "a/b".to_string();
                config
            },
            {
                let mut config = ShipperConfig::new("api");
                config.capacity = 0;
                config
            },
            {
                let mut config = ShipperConfig::new("api");
                config.batch_size = 0;
                config
            },
            {
                let mut config = ShipperConfig::new("api");
                config.capacity = 2;
                config.batch_size = 3;
                config
            },
            {
                let mut config = ShipperConfig::new("api");
                config.endpoint = Some("ftp://logs.example".to_string());
                config
            },
            {
                let mut config = ShipperConfig::new("api");
                config.endpoint = Some("http://logs.example/a b".to_string());
                config
            },
        ];
        for config in cases {
            assert_eq!(
                LogShipper::new(config).unwrap_err().kind(),
                LogErrorKind::Config
            );
        }
    }

    #[test]
    fn record_drops_debug_below_info_and_omits_scope_keys() {
        let shipper = labeled();
        shipper
            .record(event("", "hidden", LogLevel::Debug))
            .unwrap();
        assert_eq!(shipper.buffered_len(), 0);
        shipper.record(event("", "kept", LogLevel::Info)).unwrap();
        let events = shipper.buffered_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].message, "kept");
        assert_eq!(shipper.app(), "api");
        assert_eq!(shipper.project(), "demo");
        assert_no_scope_keys(&serde_json::to_string(&events[0]).unwrap());
    }

    #[test]
    fn capacity_drops_the_oldest_event() {
        let mut config = ShipperConfig::new("api");
        config.capacity = 2;
        config.batch_size = 1;
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "a", LogLevel::Info)).unwrap();
        shipper.record(event("", "b", LogLevel::Info)).unwrap();
        shipper.record(event("", "c", LogLevel::Info)).unwrap();
        let messages: Vec<_> = shipper
            .buffered_events()
            .into_iter()
            .map(|event| event.message)
            .collect();
        assert_eq!(messages, ["b", "c"]);
        assert_eq!(shipper.dropped(), 1);
    }

    #[test]
    fn record_keeps_long_message_request_id_ip_and_user_agent() {
        let shipper = buffer_only();
        let mut event = event(
            format!("GET /{}", "p".repeat(3000)),
            "é".repeat(5000),
            LogLevel::Info,
        );
        event.id = "i".repeat(200);
        event.ip = "1".repeat(80);
        event.user_agent = "é".repeat(2000);
        let message = event.message.clone();
        let action = event.action.clone();
        let id = event.id.clone();
        let ip = event.ip.clone();
        let user_agent = event.user_agent.clone();
        shipper.record(event).unwrap();
        let stored = shipper.buffered_events().remove(0);
        assert_eq!(stored.message, message);
        assert_eq!(stored.action, action);
        assert_eq!(stored.id, id);
        assert_eq!(stored.ip, ip);
        assert_eq!(stored.user_agent, user_agent);
    }

    #[tokio::test]
    async fn unset_endpoint_keeps_events_without_http() {
        let shipper = buffer_only();
        shipper.record(event("", "local", LogLevel::Info)).unwrap();
        let result = shipper.ship_batch().await.unwrap();
        assert_eq!(result.mode, ShipMode::BufferOnly);
        assert_eq!(result.shipped, 0);
        assert_eq!(result.buffered, 1);
        assert_eq!(shipper.buffered_len(), 1);
        assert_eq!(shipper.buffered_events()[0].message, "local");
    }

    #[tokio::test]
    async fn empty_batch_does_not_post() {
        let mut config = ShipperConfig::new("api");
        config.endpoint = Some("http://127.0.0.1:9".to_string());
        let shipper = LogShipper::new(config).unwrap();
        let result = shipper.ship_batch().await.unwrap();
        assert_eq!(result.mode, ShipMode::Empty);
        assert_eq!(result.shipped, 0);
    }

    #[tokio::test]
    async fn unreachable_log_server_keeps_the_buffer() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let mut config = ShipperConfig::new("api");
        config.endpoint = Some(format!("http://{addr}"));
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "down", LogLevel::Error)).unwrap();
        let err = shipper.ship_batch().await.unwrap_err();
        assert_eq!(err.kind(), LogErrorKind::Transport);
        assert_eq!(shipper.buffered_len(), 1);
        assert_eq!(shipper.buffered_events()[0].message, "down");
    }

    #[tokio::test]
    async fn ship_batch_posts_json_to_the_app_path() {
        let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
        let mut config = ShipperConfig::new("cs");
        config.project = "demo".to_string();
        config.endpoint = Some(endpoint);
        config.batch_size = 2;
        config.capacity = 10;
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "one", LogLevel::Info)).unwrap();
        shipper.record(event("", "two", LogLevel::Warn)).unwrap();
        shipper.record(event("", "three", LogLevel::Error)).unwrap();
        let first = shipper.ship_batch().await.unwrap();
        assert_eq!(first.mode, ShipMode::Sent);
        assert_eq!(first.shipped, 2);
        assert_eq!(first.buffered, 1);
        let captured = rx.recv().await.unwrap();
        assert_eq!(captured.app, "cs");
        assert_eq!(captured.path, "/log/cs");
        assert_eq!(captured.content_type, "application/json");
        assert!(!captured.has_project_header);
        let events = parse_body(&captured.body);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].message, "one");
        assert_eq!(events[1].message, "two");
        let body: serde_json::Value = serde_json::from_str(&captured.body).unwrap();
        assert!(body.as_object().is_some());
        assert!(body.as_array().is_none());
        assert_eq!(body.as_object().unwrap().len(), 1);
        for item in body["events"].as_array().unwrap() {
            assert!(item.get("app").is_none());
            assert!(item.get("project").is_none());
            assert!(item.get("action").is_some());
            assert!(item.get("request").is_none());
        }
        assert!(serde_json::from_str::<Vec<LogEvent>>(&captured.body).is_err());
        let second = shipper.ship_batch().await.unwrap();
        assert_eq!(second.shipped, 1);
        assert_eq!(shipper.buffered_len(), 0);
        let third = shipper.ship_batch().await.unwrap();
        assert_eq!(third.mode, ShipMode::Empty);
    }

    #[tokio::test]
    async fn http_error_restores_the_batch() {
        let (endpoint, _rx) = capture_server(StatusAction::Unavailable).await;
        let mut config = ShipperConfig::new("api");
        config.endpoint = Some(endpoint);
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "keep", LogLevel::Info)).unwrap();
        let err = shipper.ship_batch().await.unwrap_err();
        assert_eq!(err.kind(), LogErrorKind::Api);
        assert!(err.to_string().contains("503"));
        assert_eq!(shipper.buffered_events()[0].message, "keep");
    }

    #[tokio::test]
    async fn in_flight_records_stay_out_of_a_successful_batch() {
        let (endpoint, _rx) = capture_server(StatusAction::Slow).await;
        let mut config = ShipperConfig::new("api");
        config.endpoint = Some(endpoint);
        config.capacity = 2;
        config.batch_size = 2;
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "a", LogLevel::Info)).unwrap();
        shipper.record(event("", "b", LogLevel::Info)).unwrap();
        let shipper_task = shipper.clone();
        let ship = tokio::spawn(async move { shipper_task.ship_batch().await.unwrap() });
        tokio::time::timeout(Duration::from_secs(2), async {
            while shipper.buffered_len() != 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        shipper.record(event("", "c", LogLevel::Info)).unwrap();
        let result = ship.await.unwrap();
        assert_eq!(result.shipped, 2);
        assert_eq!(result.mode, ShipMode::Sent);
        let left = shipper.buffered_events();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].message, "c");
    }

    #[tokio::test]
    async fn failed_restore_keeps_the_oldest_batch() {
        let (endpoint, _rx) = capture_server(StatusAction::SlowUnavailable).await;
        let mut config = ShipperConfig::new("api");
        config.endpoint = Some(endpoint);
        config.capacity = 2;
        config.batch_size = 2;
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "a", LogLevel::Info)).unwrap();
        shipper.record(event("", "b", LogLevel::Info)).unwrap();
        let shipper_task = shipper.clone();
        let ship = tokio::spawn(async move { shipper_task.ship_batch().await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while shipper.buffered_len() != 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        shipper.record(event("", "c", LogLevel::Info)).unwrap();
        let err = ship.await.unwrap().unwrap_err();
        assert_eq!(err.kind(), LogErrorKind::Api);
        let left: Vec<_> = shipper
            .buffered_events()
            .into_iter()
            .map(|event| event.message)
            .collect();
        assert_eq!(left, ["a", "b"]);
        assert_eq!(shipper.dropped(), 1);
    }

    #[tokio::test]
    async fn buffer_only_loop_does_not_drop_events() {
        let shipper = buffer_only();
        shipper.record(event("", "stay", LogLevel::Info)).unwrap();
        let handle = shipper.spawn_ship_loop(Duration::from_millis(10)).unwrap();
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(!handle.is_finished());
        handle.abort();
        assert_eq!(shipper.buffered_len(), 1);
        assert_eq!(
            shipper.spawn_ship_loop(Duration::ZERO).unwrap_err().kind(),
            LogErrorKind::Config
        );
    }

    fn parse_body(body: &str) -> Vec<LogEvent> {
        #[derive(serde::Deserialize)]
        struct RequestJson {
            events: Vec<LogEvent>,
        }
        serde_json::from_str::<RequestJson>(body).unwrap().events
    }

    fn assert_no_scope_keys(line: &str) {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(value.get("app").is_none());
        assert!(value.get("project").is_none());
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
