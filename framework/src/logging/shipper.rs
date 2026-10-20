use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::event::event;
use super::{LogEvent, LogLevel};

const DEFAULT_CAPACITY: usize = 10_000;
const SHIP_CHUNK: usize = 1_000;
const SHIP_TIMEOUT: Duration = Duration::from_secs(10);
const CAPACITY_WARN_NOTE: &str = "capacity meets 80%";

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
pub struct LogBatch {
    pub events: Vec<LogEvent>,
}

impl LogBatch {
    pub fn sanitize_untrusted_client(&self) -> Option<Vec<LogEvent>> {
        if self.events.is_empty() || self.events.len() > SHIP_CHUNK {
            return None;
        }
        let mut events = Vec::with_capacity(self.events.len());
        for event in &self.events {
            events.push(event.sanitize_untrusted_client()?);
        }
        Some(events)
    }
}

#[derive(Clone)]
pub struct LogShipper {
    inner: Arc<Inner>,
}

struct Buffer {
    events: VecDeque<LogEvent>,
    capacity_warned: bool,
}

struct Inner {
    app: String,
    endpoint: Option<String>,
    capacity: usize,
    buffer: Mutex<Buffer>,
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
                buffer: Mutex::new(Buffer {
                    events: VecDeque::new(),
                    capacity_warned: false,
                }),
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
        self.lock_buffer().events.len()
    }

    pub fn record(&self, event: LogEvent) {
        let capacity = self.inner.capacity;
        let mut buffer = self.lock_buffer();
        while buffer.events.len() >= capacity {
            buffer.events.pop_front();
        }
        buffer.events.push_back(event);
        if !buffer.capacity_warned && meets_capacity_warn(buffer.events.len(), capacity) {
            while buffer.events.len() >= capacity {
                buffer.events.pop_front();
            }
            buffer.events.push_back(capacity_warn_event());
            buffer.capacity_warned = true;
        }
    }

    async fn ship_batch(&self) {
        let _shipping = self.inner.shipping.lock().await;
        let Some(endpoint) = self.inner.endpoint.clone() else {
            return;
        };
        let batch = {
            let mut buffer = self.lock_buffer();
            let count = SHIP_CHUNK.min(buffer.events.len());
            buffer.events.drain(..count).collect::<Vec<_>>()
        };
        if batch.is_empty() {
            return;
        }
        let Some(client) = self.inner.http.clone() else {
            self.restore_batch(batch);
            return;
        };
        let url = self.batch_url(&endpoint);
        let posted = match serde_json::to_string(&LogBatch {
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
        } else {
            let mut buffer = self.lock_buffer();
            if !meets_capacity_warn(buffer.events.len(), self.inner.capacity) {
                buffer.capacity_warned = false;
            }
        }
    }

    fn restore_batch(&self, batch: Vec<LogEvent>) {
        let capacity = self.inner.capacity;
        let mut buffer = self.lock_buffer();
        for event in batch.into_iter().rev() {
            buffer.events.push_front(event);
        }
        while buffer.events.len() > capacity {
            buffer.events.pop_back();
        }
    }

    fn lock_buffer(&self) -> std::sync::MutexGuard<'_, Buffer> {
        self.inner
            .buffer
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    fn batch_url(&self, endpoint: &str) -> String {
        format!("{endpoint}/log/{}", self.inner.app)
    }
}

fn meets_capacity_warn(len: usize, capacity: usize) -> bool {
    (len as u128) * 5 >= (capacity as u128) * 4
}

fn capacity_warn_event() -> LogEvent {
    event("", CAPACITY_WARN_NOTE, LogLevel::Warn)
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::event::event;
    use super::*;
    use crate::logging::LogLevel;

    pub(crate) fn buffered_events(shipper: &LogShipper) -> Vec<LogEvent> {
        shipper.lock_buffer().events.iter().cloned().collect()
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
        assert_eq!(SHIP_CHUNK, 1_000);
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
        assert_eq!(messages, [CAPACITY_WARN_NOTE.to_string(), "c".to_string()]);
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
    async fn ship_batch_posts_one_chunk_and_leaves_the_rest() {
        let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
        let shipper = LogShipper::new(ShipperConfig::new("api", endpoint)).unwrap();
        let total = SHIP_CHUNK + 1;
        for index in 0..total {
            shipper.record(event("", index.to_string(), LogLevel::Info));
        }
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 1);
        let captured = rx.recv().await.unwrap();
        let events = parse_body(&captured.body);
        assert_eq!(events.len(), SHIP_CHUNK);
        assert_eq!(events[0].message, "0");
        assert_eq!(events[SHIP_CHUNK - 1].message, (SHIP_CHUNK - 1).to_string());
        assert_eq!(buffered_events(&shipper)[0].message, SHIP_CHUNK.to_string());
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 0);
        let rest = rx.recv().await.unwrap();
        let rest_events = parse_body(&rest.body);
        assert_eq!(rest_events.len(), 1);
        assert_eq!(rest_events[0].message, SHIP_CHUNK.to_string());
    }

    #[test]
    fn sanitize_untrusted_client_accepts_a_batch_at_the_ship_chunk() {
        let body = LogBatch {
            events: (0..SHIP_CHUNK)
                .map(|_| event("/ok", "kept", LogLevel::Info))
                .collect(),
        };
        let stored = body.sanitize_untrusted_client().unwrap();
        assert_eq!(stored.len(), SHIP_CHUNK);
        assert_eq!(stored[0].message, "kept");
        assert_eq!(stored[0].id, "");
    }

    #[test]
    fn sanitize_untrusted_client_rejects_a_batch_above_the_ship_chunk() {
        let body = LogBatch {
            events: (0..=SHIP_CHUNK)
                .map(|_| event("/ok", "kept", LogLevel::Info))
                .collect(),
        };
        assert_eq!(body.events.len(), SHIP_CHUNK + 1);
        assert!(body.sanitize_untrusted_client().is_none());
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
        assert_eq!(left, ["b".to_string(), CAPACITY_WARN_NOTE.to_string()]);
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

    #[test]
    fn capacity_warn_limit_is_80_percent() {
        assert!(!meets_capacity_warn(0, 1));
        assert!(meets_capacity_warn(1, 1));
        assert!(!meets_capacity_warn(3, 4));
        assert!(meets_capacity_warn(4, 4));
        assert!(!meets_capacity_warn(3, 5));
        assert!(meets_capacity_warn(4, 5));
        assert!(!meets_capacity_warn(7999, 10_000));
        assert!(meets_capacity_warn(8000, 10_000));
    }

    #[test]
    fn record_inserts_one_capacity_warning_at_80_percent() {
        let mut config = ShipperConfig::new("api", "");
        config.capacity = 5;
        let shipper = LogShipper::new(config).unwrap();
        let mut oldest = event("GET /a", "first", LogLevel::Error);
        oldest.id = "id-1".to_string();
        oldest.ip = "203.0.113.9".to_string();
        oldest.user_agent = "agent".to_string();
        oldest.status = Some(500);
        oldest.latency_ms = Some(9);
        oldest.extra.insert("k".to_string(), serde_json::json!("v"));
        oldest.ts = chrono::DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let ts = oldest.ts;
        shipper.record(oldest);
        shipper.record(event("", "second", LogLevel::Info));
        shipper.record(event("", "third", LogLevel::Info));
        let below = buffered_events(&shipper);
        assert_eq!(below.len(), 3);
        assert_eq!(capacity_warn_count(&below), 0);
        assert_eq!(below[0].level, LogLevel::Error);
        assert_eq!(below[0].message, "first");
        let before = chrono::Utc::now();
        shipper.record(event("", "fourth", LogLevel::Info));
        let after = chrono::Utc::now();
        let events = buffered_events(&shipper);
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].level, LogLevel::Error);
        assert_eq!(events[0].message, "first");
        assert_eq!(events[0].ts, ts);
        assert_eq!(events[0].id, "id-1");
        assert_eq!(events[0].ip, "203.0.113.9");
        assert_eq!(events[0].user_agent, "agent");
        assert_eq!(events[0].action, "GET /a");
        assert_eq!(events[0].status, Some(500));
        assert_eq!(events[0].latency_ms, Some(9));
        assert_eq!(
            events[0].extra.get("k").and_then(|value| value.as_str()),
            Some("v")
        );
        assert_eq!(events[1].message, "second");
        assert_eq!(events[1].level, LogLevel::Info);
        assert_eq!(events[2].message, "third");
        assert_eq!(events[3].message, "fourth");
        assert_eq!(events[3].level, LogLevel::Info);
        assert_eq!(capacity_warn_count(&events), 1);
        assert_capacity_warn(&events[4]);
        assert!(events[4].ts >= before && events[4].ts <= after);
        shipper.record(event("", "fifth", LogLevel::Info));
        let events = buffered_events(&shipper);
        assert_eq!(
            messages_of(&events),
            ["second", "third", "fourth", CAPACITY_WARN_NOTE, "fifth"]
        );
        assert_eq!(capacity_warn_count(&events), 1);
        shipper.record(event("", "sixth", LogLevel::Info));
        let events = buffered_events(&shipper);
        assert_eq!(
            messages_of(&events),
            ["third", "fourth", CAPACITY_WARN_NOTE, "fifth", "sixth"]
        );
        assert_eq!(capacity_warn_count(&events), 1);
    }

    #[test]
    fn record_drops_the_oldest_event_to_store_the_capacity_warning() {
        let mut config = ShipperConfig::new("api", "");
        config.capacity = 4;
        let shipper = LogShipper::new(config).unwrap();
        for message in ["a", "b", "c"] {
            shipper.record(event("", message, LogLevel::Info));
        }
        let below = buffered_events(&shipper);
        assert_eq!(messages_of(&below), ["a", "b", "c"]);
        assert_eq!(capacity_warn_count(&below), 0);
        shipper.record(event("", "d", LogLevel::Info));
        let events = buffered_events(&shipper);
        assert_eq!(messages_of(&events), ["b", "c", "d", CAPACITY_WARN_NOTE]);
        assert_eq!(events[0].level, LogLevel::Info);
        assert_eq!(events[1].level, LogLevel::Info);
        assert_eq!(events[2].level, LogLevel::Info);
        assert_capacity_warn(&events[3]);
        shipper.record(event("", "e", LogLevel::Info));
        let events = buffered_events(&shipper);
        assert_eq!(messages_of(&events), ["c", "d", CAPACITY_WARN_NOTE, "e"]);
        assert_eq!(capacity_warn_count(&events), 1);
    }

    #[test]
    fn record_at_capacity_one_stores_only_the_capacity_warning() {
        let mut config = ShipperConfig::new("api", "");
        config.capacity = 1;
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("GET /", "hello", LogLevel::Info));
        let events = buffered_events(&shipper);
        assert_eq!(events.len(), 1);
        assert_capacity_warn(&events[0]);
        shipper.record(event("", "next", LogLevel::Info));
        let events = buffered_events(&shipper);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].message, "next");
        assert_eq!(events[0].level, LogLevel::Info);
        assert_eq!(capacity_warn_count(&events), 0);
    }

    #[tokio::test]
    async fn failed_ship_does_not_repeat_the_capacity_warning() {
        let (endpoint, _rx) = capture_server(StatusAction::Unavailable).await;
        let mut config = ShipperConfig::new("api", endpoint);
        config.capacity = 5;
        let shipper = LogShipper::new(config).unwrap();
        for message in ["a", "b", "c", "d"] {
            shipper.record(event("", message, LogLevel::Info));
        }
        shipper.ship_batch().await;
        let events = buffered_events(&shipper);
        assert_eq!(
            messages_of(&events),
            ["a", "b", "c", "d", CAPACITY_WARN_NOTE]
        );
        assert_eq!(events[0].level, LogLevel::Info);
        assert_capacity_warn(&events[4]);
        shipper.record(event("", "e", LogLevel::Info));
        let events = buffered_events(&shipper);
        assert_eq!(
            messages_of(&events),
            ["b", "c", "d", CAPACITY_WARN_NOTE, "e"]
        );
        assert_eq!(capacity_warn_count(&events), 1);
        assert_eq!(events[0].message, "b");
        assert_eq!(events[0].level, LogLevel::Info);
    }

    #[tokio::test]
    async fn successful_ship_below_80_percent_warns_on_the_next_crossing() {
        let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
        let mut config = ShipperConfig::new("api", endpoint);
        config.capacity = 5;
        let shipper = LogShipper::new(config).unwrap();
        for message in ["a", "b", "c", "d"] {
            shipper.record(event("", message, LogLevel::Info));
        }
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 0);
        let posted = parse_body(&rx.recv().await.unwrap().body);
        assert_eq!(
            messages_of(&posted),
            ["a", "b", "c", "d", CAPACITY_WARN_NOTE]
        );
        assert_eq!(posted[0].level, LogLevel::Info);
        assert_eq!(posted[3].message, "d");
        assert_capacity_warn(&posted[4]);
        for message in ["e", "f", "g"] {
            shipper.record(event("", message, LogLevel::Info));
        }
        let events = buffered_events(&shipper);
        assert_eq!(messages_of(&events), ["e", "f", "g"]);
        assert_eq!(capacity_warn_count(&events), 0);
        shipper.record(event("", "h", LogLevel::Info));
        let events = buffered_events(&shipper);
        assert_eq!(
            messages_of(&events),
            ["e", "f", "g", "h", CAPACITY_WARN_NOTE]
        );
        assert_eq!(events[0].level, LogLevel::Info);
        assert_capacity_warn(&events[4]);
        shipper.record(event("", "i", LogLevel::Info));
        let events = buffered_events(&shipper);
        assert_eq!(
            messages_of(&events),
            ["f", "g", "h", CAPACITY_WARN_NOTE, "i"]
        );
        assert_eq!(capacity_warn_count(&events), 1);
    }

    #[tokio::test]
    async fn successful_ship_at_80_percent_does_not_warn_again() {
        let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
        let mut config = ShipperConfig::new("api", endpoint);
        config.capacity = 5_000;
        let shipper = LogShipper::new(config).unwrap();
        for index in 0..4_000 {
            shipper.record(event("", index.to_string(), LogLevel::Info));
        }
        let events = buffered_events(&shipper);
        assert_eq!(events.len(), 4_001);
        assert_eq!(events[0].message, "0");
        assert_eq!(events[0].level, LogLevel::Info);
        assert_eq!(capacity_warn_count(&events), 1);
        assert_capacity_warn(events.last().unwrap());
        for index in 4_000..4_999 {
            shipper.record(event("", index.to_string(), LogLevel::Info));
        }
        assert_eq!(shipper.buffered_len(), 5_000);
        assert_eq!(capacity_warn_count(&buffered_events(&shipper)), 1);
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 4_000);
        let posted = parse_body(&rx.recv().await.unwrap().body);
        assert_eq!(posted.len(), SHIP_CHUNK);
        assert_eq!(posted[0].message, "0");
        assert_eq!(posted[0].level, LogLevel::Info);
        assert_eq!(capacity_warn_count(&posted), 0);
        let left = buffered_events(&shipper);
        assert_eq!(left[0].message, "1000");
        assert_eq!(left[0].level, LogLevel::Info);
        assert_eq!(capacity_warn_count(&left), 1);
        shipper.record(event("", "more", LogLevel::Info));
        let left = buffered_events(&shipper);
        assert_eq!(left.len(), 4_001);
        assert_eq!(left[0].message, "1000");
        assert_eq!(left.last().unwrap().message, "more");
        assert_eq!(left.last().unwrap().level, LogLevel::Info);
        assert_eq!(capacity_warn_count(&left), 1);
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 3_001);
        let second = parse_body(&rx.recv().await.unwrap().body);
        assert_eq!(second[0].message, "1000");
        assert_eq!(capacity_warn_count(&second), 0);
        assert_eq!(capacity_warn_count(&buffered_events(&shipper)), 1);
        for index in 0..998 {
            shipper.record(event("", format!("extra-{index}"), LogLevel::Info));
        }
        assert_eq!(shipper.buffered_len(), 3_999);
        assert_eq!(capacity_warn_count(&buffered_events(&shipper)), 1);
        shipper.record(event("", "cross", LogLevel::Info));
        let events = buffered_events(&shipper);
        assert_eq!(events.len(), 4_001);
        assert_eq!(capacity_warn_count(&events), 2);
        assert_eq!(events[events.len() - 2].message, "cross");
        assert_eq!(events[events.len() - 2].level, LogLevel::Info);
        assert_capacity_warn(events.last().unwrap());
    }

    fn messages_of(events: &[LogEvent]) -> Vec<&str> {
        events.iter().map(|event| event.message.as_str()).collect()
    }

    fn capacity_warn_count(events: &[LogEvent]) -> usize {
        events
            .iter()
            .filter(|event| event.message == CAPACITY_WARN_NOTE)
            .count()
    }

    fn assert_capacity_warn(event: &LogEvent) {
        assert_eq!(event.level, LogLevel::Warn);
        assert_eq!(event.message, CAPACITY_WARN_NOTE);
        assert_eq!(event.id, "");
        assert_eq!(event.ip, "");
        assert_eq!(event.user_agent, "");
        assert_eq!(event.action, "");
        assert_eq!(event.status, None);
        assert_eq!(event.latency_ms, None);
        assert!(event.extra.is_empty());
    }

    fn parse_body(body: &str) -> Vec<LogEvent> {
        serde_json::from_str::<LogBatch>(body).unwrap().events
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
