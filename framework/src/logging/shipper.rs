use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::LogEvent;

const DEFAULT_CAPACITY: usize = 10_000;
pub const MAX_EVENTS: usize = DEFAULT_CAPACITY;
pub const MAX_BYTES: usize = DEFAULT_CAPACITY;
const SHIP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipperConfig {
    pub app: String,
    pub endpoint: Option<String>,
    pub capacity: usize,
}

impl ShipperConfig {
    pub fn new(app: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self {
            app: app.into(),
            endpoint: Some(endpoint.into()),
            capacity: DEFAULT_CAPACITY,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestJson {
    pub events: Vec<LogEvent>,
}

impl RequestJson {
    pub fn sanitize_untrusted_client(&self) -> Option<Vec<LogEvent>> {
        if self.events.is_empty() || self.events.len() > MAX_EVENTS {
            return None;
        }
        let mut events = Vec::with_capacity(self.events.len());
        let mut bytes = 0usize;
        for event in &self.events {
            let stored = event.sanitize_untrusted_client()?;
            bytes = bytes.saturating_add(super::event::json_byte_len(&stored));
            if bytes > MAX_BYTES {
                return None;
            }
            events.push(stored);
        }
        Some(events)
    }
}

#[derive(Clone)]
pub struct LogShipper {
    inner: Arc<Inner>,
}

struct Inner {
    app: String,
    endpoint: Option<String>,
    capacity: usize,
    buffer: Mutex<VecDeque<LogEvent>>,
    http: Option<reqwest::Client>,
    shipping: tokio::sync::Mutex<()>,
}

impl std::fmt::Debug for LogShipper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogShipper")
            .field("app", &self.inner.app)
            .field("endpoint", &self.inner.endpoint)
            .field("buffered", &self.buffered_len())
            .finish_non_exhaustive()
    }
}

impl LogShipper {
    pub fn new(config: ShipperConfig) -> Option<Self> {
        let mut chars = config.app.chars();
        let first = chars.next()?;
        if !first.is_ascii_alphanumeric()
            || !config
                .app
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
            || config.capacity == 0
        {
            return None;
        }
        let endpoint = match config.endpoint {
            None => None,
            Some(raw) => {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    None
                } else if trimmed.chars().any(char::is_whitespace) {
                    return None;
                } else {
                    let parsed = reqwest::Url::parse(trimmed).ok()?;
                    if (parsed.scheme() != "http" && parsed.scheme() != "https")
                        || parsed.host_str().is_none()
                        || parsed.query().is_some()
                        || parsed.fragment().is_some()
                    {
                        return None;
                    }
                    Some(parsed.to_string().trim_end_matches('/').to_string())
                }
            }
        };
        let http = match &endpoint {
            Some(_) => Some(
                reqwest::Client::builder()
                    .connect_timeout(SHIP_TIMEOUT)
                    .timeout(SHIP_TIMEOUT)
                    .redirect(reqwest::redirect::Policy::none())
                    .user_agent(concat!("ian_framework/", env!("CARGO_PKG_VERSION")))
                    .build()
                    .ok()?,
            ),
            None => None,
        };
        Some(Self {
            inner: Arc::new(Inner {
                app: config.app,
                endpoint,
                capacity: config.capacity,
                buffer: Mutex::new(VecDeque::new()),
                http,
                shipping: tokio::sync::Mutex::new(()),
            }),
        })
    }

    pub fn spawn_ship_loop(&self, every: Duration) -> Option<tokio::task::JoinHandle<()>> {
        if every.is_zero() {
            return None;
        }
        let shipper = self.clone();
        Some(tokio::spawn(async move {
            let mut ticker = tokio::time::interval(every);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                shipper.ship_batch().await;
            }
        }))
    }

    fn buffered_len(&self) -> usize {
        self.lock_buffer().len()
    }

    pub fn record(&self, event: LogEvent) {
        let capacity = self.inner.capacity;
        let mut buffer = self.lock_buffer();
        while buffer.len() >= capacity {
            buffer.pop_front();
        }
        buffer.push_back(event);
    }

    pub fn record_untrusted(&self, event: LogEvent) -> bool {
        let Some(event) = event.sanitize_untrusted_client() else {
            return false;
        };
        self.record(event);
        true
    }

    pub fn record_client_batch(&self, body: &RequestJson) -> bool {
        let Some(events) = body.sanitize_untrusted_client() else {
            return false;
        };
        for event in events {
            self.record(event);
        }
        true
    }

    async fn ship_batch(&self) {
        let _shipping = self.inner.shipping.lock().await;
        let Some(endpoint) = self.inner.endpoint.clone() else {
            return;
        };
        let batch = {
            let mut buffer = self.lock_buffer();
            buffer.drain(..).collect::<Vec<_>>()
        };
        if batch.is_empty() {
            return;
        }
        let Some(client) = self.inner.http.clone() else {
            self.restore_batch(batch);
            return;
        };
        let url = self.batch_url(&endpoint);
        let posted = match serde_json::to_string(&RequestJson {
            events: batch.clone(),
        }) {
            Ok(body) => match client
                .post(&url)
                .timeout(SHIP_TIMEOUT)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body)
                .send()
                .await
            {
                Ok(response) => response.status().is_success(),
                Err(_) => false,
            },
            Err(_) => false,
        };
        if !posted {
            self.restore_batch(batch);
        }
    }

    fn restore_batch(&self, batch: Vec<LogEvent>) {
        let capacity = self.inner.capacity;
        let mut buffer = self.lock_buffer();
        for event in batch.into_iter().rev() {
            buffer.push_front(event);
        }
        while buffer.len() > capacity {
            buffer.pop_back();
        }
    }

    fn lock_buffer(&self) -> std::sync::MutexGuard<'_, VecDeque<LogEvent>> {
        self.inner
            .buffer
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    fn batch_url(&self, endpoint: &str) -> String {
        format!("{endpoint}/log/{}", self.inner.app)
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::event::event;
    use super::*;
    use crate::logging::LogLevel;
    use chrono::{DateTime, Utc};
    use serde_json::json;

    pub(crate) fn buffered_events(shipper: &LogShipper) -> Vec<LogEvent> {
        shipper.lock_buffer().iter().cloned().collect()
    }

    fn buffer_only() -> LogShipper {
        LogShipper::new(ShipperConfig::new("api", "")).unwrap()
    }

    #[test]
    fn defaults_are_buffer_only() {
        let shipper = buffer_only();
        assert_eq!(shipper.inner.app, "api");
        assert_eq!(shipper.inner.endpoint, None);
        assert_eq!(DEFAULT_CAPACITY, 10_000);
        assert_eq!(MAX_EVENTS, DEFAULT_CAPACITY);
        assert_eq!(MAX_BYTES, DEFAULT_CAPACITY);
        assert_eq!(crate::logging::MAX_EVENTS, MAX_EVENTS);
        assert_eq!(crate::logging::MAX_BYTES, MAX_BYTES);
        assert_eq!(SHIP_TIMEOUT, Duration::from_secs(10));
    }

    #[test]
    fn blank_endpoint_is_buffer_only() {
        for endpoint in ["", "   "] {
            let shipper = LogShipper::new(ShipperConfig::new("api", endpoint)).unwrap();
            assert_eq!(shipper.inner.endpoint, None);
        }
    }

    #[test]
    fn endpoint_is_normalized() {
        let shipper =
            LogShipper::new(ShipperConfig::new("cs", "http://logs.example/ingest/")).unwrap();
        assert_eq!(
            shipper.inner.endpoint.as_deref(),
            Some("http://logs.example/ingest")
        );
        assert_eq!(
            shipper.batch_url(shipper.inner.endpoint.as_deref().unwrap()),
            "http://logs.example/ingest/log/cs"
        );
    }

    #[test]
    fn invalid_config_is_rejected() {
        let cases = [
            ShipperConfig::new("", "http://logs.example"),
            ShipperConfig::new("a/b", "http://logs.example"),
            {
                let mut config = ShipperConfig::new("api", "http://logs.example");
                config.capacity = 0;
                config
            },
            ShipperConfig::new("api", "ftp://logs.example"),
            ShipperConfig::new("api", "http://logs.example/a b"),
        ];
        for config in cases {
            assert!(LogShipper::new(config).is_none());
        }
    }

    #[test]
    fn record_omits_scope_keys() {
        let shipper = buffer_only();
        shipper.record(event("", "kept", LogLevel::Info));
        let events = buffered_events(&shipper);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].message, "kept");
        assert_eq!(shipper.inner.app, "api");
        assert_no_scope_keys(&serde_json::to_string(&events[0]).unwrap());
    }

    #[test]
    fn capacity_drops_the_oldest_event() {
        let mut config = ShipperConfig::new("api", "");
        config.capacity = 2;
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "a", LogLevel::Info));
        shipper.record(event("", "b", LogLevel::Info));
        shipper.record(event("", "c", LogLevel::Info));
        let messages: Vec<_> = buffered_events(&shipper)
            .into_iter()
            .map(|event| event.message)
            .collect();
        assert_eq!(messages, ["b", "c"]);
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
        shipper.record(event);
        let stored = buffered_events(&shipper).remove(0);
        assert_eq!(stored.message, message);
        assert_eq!(stored.action, action);
        assert_eq!(stored.id, id);
        assert_eq!(stored.ip, ip);
        assert_eq!(stored.user_agent, user_agent);
    }

    #[test]
    fn record_keeps_an_event_above_the_untrusted_byte_limit() {
        let shipper = buffer_only();
        let mut trusted = event("GET /api/files", "m".repeat(MAX_BYTES + 50), LogLevel::Info);
        trusted.ip = "203.0.113.8".to_string();
        trusted.user_agent = "demo-agent".to_string();
        trusted.status = Some(204);
        trusted.latency_ms = Some(12);
        let message = trusted.message.clone();
        shipper.record(trusted);
        let stored = buffered_events(&shipper).remove(0);
        assert_eq!(stored.message, message);
        assert_eq!(stored.ip, "203.0.113.8");
        assert_eq!(stored.user_agent, "demo-agent");
        assert_eq!(stored.status, Some(204));
        assert_eq!(stored.latency_ms, Some(12));
    }

    #[test]
    fn record_untrusted_rejects_a_blank_message() {
        let shipper = buffer_only();
        shipper.record(event("GET /health", "request", LogLevel::Info));
        for message in ["", " ", "\n", "\t", " \r\n "] {
            assert!(
                !shipper.record_untrusted(event("/ok", message, LogLevel::Info)),
                "{message:?}"
            );
        }
        let left = buffered_events(&shipper);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].message, "request");
        assert_eq!(left[0].action, "GET /health");
    }

    #[test]
    fn record_client_batch_rejects_a_bad_batch() {
        let shipper = buffer_only();
        shipper.record(event("GET /health", "request", LogLevel::Info));
        assert!(!shipper.record_client_batch(&RequestJson { events: vec![] }));
        assert!(!shipper.record_client_batch(&RequestJson {
            events: vec![
                event("/ok", "kept", LogLevel::Info),
                event("/ok", "   ", LogLevel::Warn),
                event("/ok", "also", LogLevel::Error),
            ],
        }));
        assert!(!shipper.record_untrusted(event("/ok", "m".repeat(MAX_BYTES), LogLevel::Info,)));
        assert!(!shipper.record_client_batch(&RequestJson {
            events: (0..=MAX_EVENTS)
                .map(|_| event("/ok", "m", LogLevel::Info))
                .collect(),
        }));
        let wide = "m".repeat(6_000);
        assert!(!shipper.record_client_batch(&RequestJson {
            events: vec![
                event("/a", wide.clone(), LogLevel::Info),
                event("/b", wide, LogLevel::Warn),
            ],
        }));
        let left = buffered_events(&shipper);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].message, "request");
        assert_eq!(left[0].action, "GET /health");
    }

    #[tokio::test]
    async fn unset_endpoint_keeps_events_without_http() {
        let shipper = buffer_only();
        shipper.record(event("", "local", LogLevel::Info));
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 1);
        assert_eq!(buffered_events(&shipper)[0].message, "local");
    }

    #[tokio::test]
    async fn empty_batch_does_not_post() {
        let config = ShipperConfig::new("api", "http://127.0.0.1:9");
        let shipper = LogShipper::new(config).unwrap();
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 0);
    }

    #[tokio::test]
    async fn unreachable_log_server_keeps_the_buffer() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let config = ShipperConfig::new("api", format!("http://{addr}"));
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "down", LogLevel::Error));
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 1);
        assert_eq!(buffered_events(&shipper)[0].message, "down");
    }

    #[tokio::test]
    async fn ship_batch_posts_json_to_the_app_path() {
        let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
        let config = ShipperConfig::new("cs", endpoint);
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "one", LogLevel::Info));
        shipper.record(event("", "two", LogLevel::Warn));
        shipper.record(event("", "three", LogLevel::Error));
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 0);
        let captured = rx.recv().await.unwrap();
        assert_eq!(captured.app, "cs");
        assert_eq!(captured.path, "/log/cs");
        assert_eq!(captured.content_type, "application/json");
        let events = parse_body(&captured.body);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].message, "one");
        assert_eq!(events[1].message, "two");
        assert_eq!(events[2].message, "three");
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
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 0);
    }

    #[tokio::test]
    async fn record_ships_middleware_fields() {
        let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
        let shipper = LogShipper::new(ShipperConfig::new("cs", endpoint)).unwrap();
        let trusted_ts = DateTime::parse_from_rfc3339("2026-10-18T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut trusted = event("GET /api/files", "request", LogLevel::Info);
        trusted.ts = trusted_ts;
        trusted.id = "req-1".to_string();
        trusted.ip = "203.0.113.8".to_string();
        trusted.user_agent = "demo-agent".to_string();
        trusted.status = Some(204);
        trusted.latency_ms = Some(12);
        shipper.record(trusted);
        shipper.ship_batch().await;
        let captured = rx.recv().await.unwrap();
        assert_eq!(captured.path, "/log/cs");
        let posted = parse_body(&captured.body);
        assert_eq!(posted.len(), 1);
        assert_eq!(posted[0].ts, trusted_ts);
        assert_eq!(posted[0].id, "req-1");
        assert_eq!(posted[0].ip, "203.0.113.8");
        assert_eq!(posted[0].user_agent, "demo-agent");
        assert_eq!(posted[0].action, "GET /api/files");
        assert_eq!(posted[0].status, Some(204));
        assert_eq!(posted[0].latency_ms, Some(12));
        assert_eq!(posted[0].level, LogLevel::Info);
        assert_eq!(posted[0].message, "request");
    }

    #[tokio::test]
    async fn record_untrusted_sanitizes_before_post() {
        let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
        let shipper = LogShipper::new(ShipperConfig::new("cs", endpoint)).unwrap();
        let mut client = event("GET /download?sig=abc", "  saved file  ", LogLevel::Warn);
        client.ts = DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        client.id = "client-id".to_string();
        client.ip = "198.51.100.9".to_string();
        client.user_agent = "browser".to_string();
        client.status = Some(500);
        client.latency_ms = Some(42);
        client.extra.insert("note".to_string(), json!("ok"));
        client.extra.insert("bad key".to_string(), json!("no"));
        client.extra.insert("lines".to_string(), json!("a\nb"));
        let before = Utc::now();
        assert!(shipper.record_untrusted(client));
        let after = Utc::now();
        let buffered = buffered_events(&shipper);
        assert_eq!(buffered.len(), 1);
        assert_eq!(buffered[0].ip, "");
        assert_eq!(buffered[0].status, None);
        shipper.ship_batch().await;
        let posted = parse_body(&rx.recv().await.unwrap().body);
        assert_eq!(posted.len(), 1);
        assert_eq!(posted[0].id, "");
        assert_eq!(posted[0].ip, "");
        assert_eq!(posted[0].user_agent, "");
        assert_eq!(posted[0].status, None);
        assert_eq!(posted[0].latency_ms, None);
        assert_eq!(posted[0].action, "GET /download?sig=abc");
        assert_eq!(posted[0].message, "saved file");
        assert_eq!(posted[0].level, LogLevel::Warn);
        assert_eq!(posted[0].extra["note"], json!("ok"));
        assert!(posted[0].extra.get("bad key").is_none());
        assert!(posted[0].extra.get("lines").is_none());
        assert!(posted[0].ts >= before && posted[0].ts <= after);
    }

    #[tokio::test]
    async fn record_client_batch_sanitizes_before_post() {
        let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
        let shipper = LogShipper::new(ShipperConfig::new("cs", endpoint)).unwrap();
        let mut trusted = event("POST /api/files", "request", LogLevel::Warn);
        let trusted_ts = DateTime::parse_from_rfc3339("2026-10-18T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        trusted.ts = trusted_ts;
        trusted.id = "req-9".to_string();
        trusted.ip = "203.0.113.8".to_string();
        trusted.user_agent = "demo-agent".to_string();
        trusted.status = Some(400);
        trusted.latency_ms = Some(30);
        shipper.record(trusted);

        let mut first = event("/a?x=1", " one ", LogLevel::Info);
        first.id = "c1".to_string();
        first.ip = "203.0.113.1".to_string();
        first.user_agent = "ua-1".to_string();
        first.status = Some(201);
        first.latency_ms = Some(3);
        first.ts = DateTime::parse_from_rfc3339("1999-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut second = event("#frag", "two", LogLevel::Error);
        second.id = "c2".to_string();
        second.ip = "203.0.113.2".to_string();
        second.user_agent = "ua-2".to_string();
        second.status = Some(500);
        second.latency_ms = Some(9);
        let before = Utc::now();
        assert!(shipper.record_client_batch(&RequestJson {
            events: vec![first, second],
        }));
        let after = Utc::now();
        shipper.ship_batch().await;
        let posted = parse_body(&rx.recv().await.unwrap().body);
        assert_eq!(posted.len(), 3);
        assert_eq!(posted[0].ts, trusted_ts);
        assert_eq!(posted[0].id, "req-9");
        assert_eq!(posted[0].ip, "203.0.113.8");
        assert_eq!(posted[0].user_agent, "demo-agent");
        assert_eq!(posted[0].status, Some(400));
        assert_eq!(posted[0].latency_ms, Some(30));
        assert_eq!(posted[0].action, "POST /api/files");
        assert_eq!(posted[0].message, "request");
        assert_eq!(posted[1].action, "/a?x=1");
        assert_eq!(posted[1].message, "one");
        assert_eq!(posted[1].level, LogLevel::Info);
        assert_eq!(posted[1].id, "");
        assert_eq!(posted[1].ip, "");
        assert_eq!(posted[1].user_agent, "");
        assert_eq!(posted[1].status, None);
        assert_eq!(posted[1].latency_ms, None);
        assert!(posted[1].ts >= before && posted[1].ts <= after);
        assert_eq!(posted[2].action, "#frag");
        assert_eq!(posted[2].message, "two");
        assert_eq!(posted[2].level, LogLevel::Error);
        assert_eq!(posted[2].id, "");
        assert_eq!(posted[2].ip, "");
        assert_eq!(posted[2].user_agent, "");
        assert_eq!(posted[2].status, None);
        assert_eq!(posted[2].latency_ms, None);
    }

    #[tokio::test]
    async fn http_error_restores_the_batch() {
        let (endpoint, _rx) = capture_server(StatusAction::Unavailable).await;
        let shipper = LogShipper::new(ShipperConfig::new("api", endpoint)).unwrap();
        shipper.record(event("", "keep", LogLevel::Info));
        shipper.ship_batch().await;
        assert_eq!(buffered_events(&shipper)[0].message, "keep");
    }

    #[tokio::test]
    async fn in_flight_records_stay_out_of_a_successful_batch() {
        let (endpoint, _rx) = capture_server(StatusAction::Slow).await;
        let mut config = ShipperConfig::new("api", endpoint);
        config.capacity = 2;
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "a", LogLevel::Info));
        shipper.record(event("", "b", LogLevel::Info));
        let shipper_task = shipper.clone();
        let ship = tokio::spawn(async move { shipper_task.ship_batch().await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while shipper.buffered_len() != 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        shipper.record(event("", "c", LogLevel::Info));
        ship.await.unwrap();
        let left = buffered_events(&shipper);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].message, "c");
    }

    #[tokio::test]
    async fn failed_restore_keeps_the_oldest_batch() {
        let (endpoint, _rx) = capture_server(StatusAction::SlowUnavailable).await;
        let mut config = ShipperConfig::new("api", endpoint);
        config.capacity = 2;
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "a", LogLevel::Info));
        shipper.record(event("", "b", LogLevel::Info));
        let shipper_task = shipper.clone();
        let ship = tokio::spawn(async move { shipper_task.ship_batch().await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while shipper.buffered_len() != 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        shipper.record(event("", "c", LogLevel::Info));
        ship.await.unwrap();
        let left: Vec<_> = buffered_events(&shipper)
            .into_iter()
            .map(|event| event.message)
            .collect();
        assert_eq!(left, ["a", "b"]);
    }

    #[tokio::test]
    async fn buffer_only_loop_does_not_drop_events() {
        let shipper = buffer_only();
        shipper.record(event("", "stay", LogLevel::Info));
        let handle = shipper.spawn_ship_loop(Duration::from_millis(10)).unwrap();
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(!handle.is_finished());
        handle.abort();
        assert_eq!(shipper.buffered_len(), 1);
        assert!(shipper.spawn_ship_loop(Duration::ZERO).is_none());
    }

    fn parse_body(body: &str) -> Vec<LogEvent> {
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
