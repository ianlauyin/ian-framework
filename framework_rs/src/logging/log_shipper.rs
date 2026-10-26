use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::shutdown::ShutdownReceiver;

use super::{event, LogBatch, LogEvent, LogLevel, ShipperConfig, SHIP_CHUNK};

const SHIP_TIMEOUT: Duration = Duration::from_secs(10);
const CAPACITY_WARN_NOTE: &str = "capacity meets 80%";
const MASK: &str = "***";

#[derive(Clone)]
pub struct LogShipper {
    inner: Arc<Inner>,
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
            || config.ship_every.is_zero()
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
                        || !parsed.username().is_empty()
                        || parsed.password().is_some()
                        || parsed.query().is_some()
                        || parsed.fragment().is_some()
                    {
                        return None;
                    }
                    Some(parsed.to_string().trim_end_matches('/').to_string())
                }
            }
        };
        let masks = config
            .mask_patterns
            .iter()
            .map(|pattern| regex::Regex::new(pattern).ok())
            .collect::<Option<Vec<_>>>()?;
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
        if endpoint.is_some() {
            tracing::info!(app = %config.app, "log shipping posts to the log server");
        } else {
            tracing::info!(app = %config.app, "log shipping is trace-only");
        }
        Some(Self {
            inner: Arc::new(Inner {
                app: config.app,
                endpoint,
                capacity: config.capacity,
                ship_every: config.ship_every,
                masks,
                buffer: Mutex::new(Buffer {
                    events: VecDeque::new(),
                    capacity_warned: false,
                }),
                http,
                shipping: tokio::sync::Mutex::new(()),
            }),
        })
    }

    pub fn record(&self, mut event: LogEvent) {
        mask_event(&self.inner.masks, &mut event);
        if self.inner.endpoint.is_none() {
            trace_event(&self.inner.app, &event);
            return;
        }
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

    pub fn spawn_ship_loop(&self, shutdown: ShutdownReceiver) -> tokio::task::JoinHandle<()> {
        let shipper = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(shipper.inner.ship_every);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let stop = shutdown.wait();
            tokio::pin!(stop);
            loop {
                tokio::select! {
                    biased;
                    _ = &mut stop => break,
                    _ = ticker.tick() => shipper.ship_batch().await,
                }
            }
            shipper.flush().await;
        })
    }

    fn lock_buffer(&self) -> std::sync::MutexGuard<'_, Buffer> {
        self.inner
            .buffer
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    fn buffered_len(&self) -> usize {
        self.lock_buffer().events.len()
    }

    #[cfg(test)]
    pub(crate) fn buffered_events(&self) -> Vec<LogEvent> {
        self.lock_buffer().events.iter().cloned().collect()
    }

    // Stop at the first batch that does not leave the buffer, so a down log
    // server does not hold the process open.
    async fn flush(&self) {
        loop {
            let before = self.buffered_len();
            if before == 0 {
                return;
            }
            self.ship_batch().await;
            if self.buffered_len() >= before {
                return;
            }
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

    fn batch_url(&self, endpoint: &str) -> String {
        format!("{endpoint}/log/{}", self.inner.app)
    }
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

struct Buffer {
    events: VecDeque<LogEvent>,
    capacity_warned: bool,
}

struct Inner {
    app: String,
    endpoint: Option<String>,
    capacity: usize,
    ship_every: Duration,
    masks: Vec<regex::Regex>,
    buffer: Mutex<Buffer>,
    http: Option<reqwest::Client>,
    shipping: tokio::sync::Mutex<()>,
}

fn meets_capacity_warn(len: usize, capacity: usize) -> bool {
    (len as u128) * 5 >= (capacity as u128) * 4
}

fn mask_event(masks: &[regex::Regex], event: &mut LogEvent) {
    if masks.is_empty() {
        return;
    }
    for (key, value) in event.extra.iter_mut() {
        mask_entry(masks, key, value);
    }
}

fn mask_entry(masks: &[regex::Regex], key: &str, value: &mut Value) {
    if masks.iter().any(|mask| mask.is_match(key)) {
        *value = Value::String(MASK.to_string());
    } else {
        mask_value(masks, value);
    }
}

fn mask_value(masks: &[regex::Regex], value: &mut Value) {
    match value {
        Value::String(text) => mask_text(masks, text),
        Value::Array(items) => items.iter_mut().for_each(|item| mask_value(masks, item)),
        Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                mask_entry(masks, key, item);
            }
        }
        _ => {}
    }
}

fn mask_text(masks: &[regex::Regex], text: &mut String) {
    for mask in masks {
        if let std::borrow::Cow::Owned(masked) = mask.replace_all(text, MASK) {
            *text = masked;
        }
    }
}

fn capacity_warn_event() -> LogEvent {
    event("", CAPACITY_WARN_NOTE, LogLevel::Warn)
}

fn trace_event(app: &str, event: &LogEvent) {
    let extra = serde_json::to_string(&event.extra).unwrap_or_default();
    macro_rules! emit {
        ($level:ident) => {
            tracing::$level!(
                app,
                id = %event.id,
                ip = %event.ip,
                user_agent = %event.user_agent,
                action = %event.action,
                status = ?event.status,
                latency_ms = ?event.latency_ms,
                extra = %extra,
                "{}",
                event.message
            )
        };
    }
    match event.level {
        LogLevel::Info => emit!(info),
        LogLevel::Warn => emit!(warn),
        LogLevel::Error => emit!(error),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use super::*;

    fn config(app: &str, endpoint: impl Into<String>) -> ShipperConfig {
        let mut config = ShipperConfig::new(app);
        config.endpoint = Some(endpoint.into());
        config
    }

    #[test]
    fn defaults() {
        let shipper = LogShipper::new(ShipperConfig::new("api")).unwrap();
        assert_eq!(shipper.inner.app, "api");
        assert_eq!(shipper.inner.masks.len(), 3);
        assert_eq!(SHIP_CHUNK, 1_000);
        assert_eq!(SHIP_TIMEOUT, Duration::from_secs(10));
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
    fn blank_endpoint_is_trace_only() {
        for endpoint in ["", "   "] {
            let shipper = LogShipper::new(config("api", endpoint)).unwrap();
            assert_eq!(shipper.inner.endpoint, None);
        }
        let mut unset = ShipperConfig::new("api");
        unset.endpoint = None;
        assert_eq!(LogShipper::new(unset).unwrap().inner.endpoint, None);
    }

    #[test]
    fn endpoint_is_normalized() {
        let shipper = LogShipper::new(config("cs", "http://logs.example/ingest/")).unwrap();
        assert_eq!(
            shipper.inner.endpoint.as_deref(),
            Some("http://logs.example/ingest")
        );
    }

    #[test]
    fn invalid_config_is_rejected() {
        let cases = [
            config("", "http://logs.example"),
            config("a/b", "http://logs.example"),
            {
                let mut config = config("api", "http://logs.example");
                config.capacity = 0;
                config
            },
            {
                let mut config = config("api", "http://logs.example");
                config.ship_every = Duration::ZERO;
                config
            },
            config("api", "ftp://logs.example"),
            config("api", "http://logs.example/a b"),
            {
                let mut config = config("api", "http://logs.example");
                config.mask_patterns = vec!["(".to_string()];
                config
            },
        ];
        for config in cases {
            assert!(LogShipper::new(config).is_none());
        }
    }

    #[test]
    fn endpoint_with_user_info_is_rejected() {
        for endpoint in [
            "https://user:pass@logs.example",
            "https://user@logs.example",
            "https://:pass@logs.example",
        ] {
            assert!(LogShipper::new(config("api", endpoint)).is_none());
        }
    }

    fn buffered() -> LogShipper {
        LogShipper::new(config("api", "http://logs.example")).unwrap()
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

    fn assert_no_scope_keys(line: &str) {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(value.get("app").is_none());
        assert!(value.get("project").is_none());
    }

    #[test]
    fn record_omits_scope_keys() {
        let shipper = buffered();
        shipper.record(event("", "kept", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].message, "kept");
        assert_eq!(shipper.inner.app, "api");
        assert_no_scope_keys(&serde_json::to_string(&events[0]).unwrap());
    }

    #[test]
    fn record_masks_every_pattern_match_in_extra_values() {
        let mut config = config("api", "http://logs.example");
        config.mask_patterns = vec![
            r"password=\S+".to_string(),
            r"(?i)bearer [A-Za-z0-9._-]+".to_string(),
        ];
        let shipper = LogShipper::new(config).unwrap();
        let mut event = event(
            "POST /login?password=x",
            "login password=hunter2 ok",
            LogLevel::Info,
        );
        event.id = "password=1".to_string();
        event.user_agent = "agent BEARER abc.def".to_string();
        event
            .extra
            .insert("password=k".to_string(), serde_json::json!("Bearer t0k"));
        event.extra.insert(
            "note".to_string(),
            serde_json::json!("user password=a, BEARER b.c done"),
        );
        event.extra.insert(
            "nested".to_string(),
            serde_json::json!({"list": ["password=a", 7], "keep": "plain"}),
        );
        shipper.record(event);
        let stored = shipper.buffered_events().remove(0);
        assert_eq!(stored.message, "login password=hunter2 ok");
        assert_eq!(stored.action, "POST /login?password=x");
        assert_eq!(stored.id, "password=1");
        assert_eq!(stored.user_agent, "agent BEARER abc.def");
        assert_eq!(stored.extra["password=k"], serde_json::json!("***"));
        assert_eq!(stored.extra["note"], serde_json::json!("user *** *** done"));
        assert_eq!(
            stored.extra["nested"],
            serde_json::json!({"list": ["***", 7], "keep": "plain"})
        );
    }

    #[test]
    fn record_masks_the_value_of_a_matching_extra_key() {
        let mut config = config("api", "http://logs.example");
        config.mask_patterns = vec![r"(?i)^(password|token)$".to_string()];
        let shipper = LogShipper::new(config).unwrap();
        let mut event = event("", "login", LogLevel::Info);
        event
            .extra
            .insert("password".to_string(), serde_json::json!("hunter2"));
        event
            .extra
            .insert("TOKEN".to_string(), serde_json::json!(1234));
        event.extra.insert(
            "nested".to_string(),
            serde_json::json!({"token": {"a": 1}, "user": "ian"}),
        );
        event
            .extra
            .insert("user".to_string(), serde_json::json!("ian"));
        shipper.record(event);
        let stored = shipper.buffered_events().remove(0);
        assert_eq!(stored.extra["password"], serde_json::json!("***"));
        assert_eq!(stored.extra["TOKEN"], serde_json::json!("***"));
        assert_eq!(
            stored.extra["nested"],
            serde_json::json!({"token": "***", "user": "ian"})
        );
        assert_eq!(stored.extra["user"], serde_json::json!("ian"));
    }

    #[test]
    fn record_without_mask_patterns_keeps_the_text() {
        let mut config = ShipperConfig::new("api");
        config.endpoint = Some("http://logs.example".to_string());
        config.mask_patterns = Vec::new();
        let shipper = LogShipper::new(config).unwrap();
        let mut event = event("", "password=hunter2", LogLevel::Info);
        event
            .extra
            .insert("note".to_string(), serde_json::json!("password=hunter2"));
        shipper.record(event);
        let stored = shipper.buffered_events().remove(0);
        assert_eq!(stored.message, "password=hunter2");
        assert_eq!(stored.extra["note"], serde_json::json!("password=hunter2"));
    }

    #[test]
    fn unset_endpoint_traces_without_buffering() {
        let shipper = LogShipper::new(config("api", "")).unwrap();
        shipper.record(event("GET /", "traced", LogLevel::Error));
        assert!(shipper.buffered_events().is_empty());
    }

    #[test]
    fn capacity_drops_the_oldest_event() {
        let mut config = config("api", "http://logs.example");
        config.capacity = 2;
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "a", LogLevel::Info));
        shipper.record(event("", "b", LogLevel::Info));
        shipper.record(event("", "c", LogLevel::Info));
        let messages: Vec<_> = shipper
            .buffered_events()
            .into_iter()
            .map(|event| event.message)
            .collect();
        assert_eq!(messages, [CAPACITY_WARN_NOTE.to_string(), "c".to_string()]);
    }

    #[test]
    fn record_keeps_long_message_request_id_ip_and_user_agent() {
        let shipper = buffered();
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
        let stored = shipper.buffered_events().remove(0);
        assert_eq!(stored.message, message);
        assert_eq!(stored.action, action);
        assert_eq!(stored.id, id);
        assert_eq!(stored.ip, ip);
        assert_eq!(stored.user_agent, user_agent);
    }

    #[test]
    fn record_inserts_one_capacity_warning_at_80_percent() {
        let mut config = config("api", "http://logs.example");
        config.capacity = 5;
        let shipper = LogShipper::new(config).unwrap();
        let mut oldest = event("GET /a", "first", LogLevel::Error);
        oldest.id = "id-1".to_string();
        oldest.ip = "203.0.113.9".to_string();
        oldest.user_agent = "agent".to_string();
        oldest.status = Some(500);
        oldest.latency_ms = Some(9);
        oldest.extra.insert("k".to_string(), serde_json::json!("v"));
        oldest.ts = DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let ts = oldest.ts;
        shipper.record(oldest);
        shipper.record(event("", "second", LogLevel::Info));
        shipper.record(event("", "third", LogLevel::Info));
        let below = shipper.buffered_events();
        assert_eq!(below.len(), 3);
        assert_eq!(capacity_warn_count(&below), 0);
        assert_eq!(below[0].level, LogLevel::Error);
        assert_eq!(below[0].message, "first");
        let before = Utc::now();
        shipper.record(event("", "fourth", LogLevel::Info));
        let after = Utc::now();
        let events = shipper.buffered_events();
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
        let events = shipper.buffered_events();
        assert_eq!(
            messages_of(&events),
            ["second", "third", "fourth", CAPACITY_WARN_NOTE, "fifth"]
        );
        assert_eq!(capacity_warn_count(&events), 1);
        shipper.record(event("", "sixth", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(
            messages_of(&events),
            ["third", "fourth", CAPACITY_WARN_NOTE, "fifth", "sixth"]
        );
        assert_eq!(capacity_warn_count(&events), 1);
    }

    #[test]
    fn record_drops_the_oldest_event_to_store_the_capacity_warning() {
        let mut config = config("api", "http://logs.example");
        config.capacity = 4;
        let shipper = LogShipper::new(config).unwrap();
        for message in ["a", "b", "c"] {
            shipper.record(event("", message, LogLevel::Info));
        }
        let below = shipper.buffered_events();
        assert_eq!(messages_of(&below), ["a", "b", "c"]);
        assert_eq!(capacity_warn_count(&below), 0);
        shipper.record(event("", "d", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(messages_of(&events), ["b", "c", "d", CAPACITY_WARN_NOTE]);
        assert_eq!(events[0].level, LogLevel::Info);
        assert_eq!(events[1].level, LogLevel::Info);
        assert_eq!(events[2].level, LogLevel::Info);
        assert_capacity_warn(&events[3]);
        shipper.record(event("", "e", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(messages_of(&events), ["c", "d", CAPACITY_WARN_NOTE, "e"]);
        assert_eq!(capacity_warn_count(&events), 1);
    }

    #[test]
    fn record_at_capacity_one_stores_only_the_capacity_warning() {
        let mut config = config("api", "http://logs.example");
        config.capacity = 1;
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("GET /", "hello", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(events.len(), 1);
        assert_capacity_warn(&events[0]);
        shipper.record(event("", "next", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].message, "next");
        assert_eq!(events[0].level, LogLevel::Info);
        assert_eq!(capacity_warn_count(&events), 0);
    }

    fn trace_only() -> LogShipper {
        LogShipper::new(config("api", "")).unwrap()
    }

    fn parse_body(body: &str) -> Vec<LogEvent> {
        serde_json::from_str::<LogBatch>(body).unwrap().events
    }

    #[test]
    fn batch_url_uses_the_app_path() {
        let shipper = LogShipper::new(config("cs", "http://logs.example/ingest/")).unwrap();
        assert_eq!(
            shipper.batch_url(shipper.inner.endpoint.as_deref().unwrap()),
            "http://logs.example/ingest/log/cs"
        );
    }

    #[tokio::test]
    async fn unset_endpoint_ships_without_http() {
        let shipper = trace_only();
        shipper.record(event("", "local", LogLevel::Info));
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 0);
        assert!(shipper.inner.http.is_none());
    }

    #[tokio::test]
    async fn empty_batch_does_not_post() {
        let config = config("api", "http://127.0.0.1:9");
        let shipper = LogShipper::new(config).unwrap();
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 0);
    }

    #[tokio::test]
    async fn unreachable_log_server_keeps_the_buffer() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let config = config("api", format!("http://{addr}"));
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "down", LogLevel::Error));
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 1);
        assert_eq!(shipper.buffered_events()[0].message, "down");
    }

    #[tokio::test]
    async fn ship_batch_posts_json_to_the_app_path() {
        let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
        let config = config("cs", endpoint);
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
        let shipper = LogShipper::new(config("api", endpoint)).unwrap();
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
        assert_eq!(shipper.buffered_events()[0].message, SHIP_CHUNK.to_string());
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 0);
        let rest = rx.recv().await.unwrap();
        let rest_events = parse_body(&rest.body);
        assert_eq!(rest_events.len(), 1);
        assert_eq!(rest_events[0].message, SHIP_CHUNK.to_string());
    }

    #[tokio::test]
    async fn http_error_restores_the_batch() {
        let (endpoint, _rx) = capture_server(StatusAction::Unavailable).await;
        let shipper = LogShipper::new(config("api", endpoint)).unwrap();
        shipper.record(event("", "keep", LogLevel::Info));
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_events()[0].message, "keep");
    }

    #[tokio::test]
    async fn in_flight_records_stay_out_of_a_successful_batch() {
        let (endpoint, _rx) = capture_server(StatusAction::Slow).await;
        let mut config = config("api", endpoint);
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
        let left = shipper.buffered_events();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].message, "c");
    }

    #[tokio::test]
    async fn failed_restore_keeps_the_oldest_batch() {
        let (endpoint, _rx) = capture_server(StatusAction::SlowUnavailable).await;
        let mut config = config("api", endpoint);
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
        let left: Vec<_> = shipper
            .buffered_events()
            .into_iter()
            .map(|event| event.message)
            .collect();
        assert_eq!(left, ["b".to_string(), CAPACITY_WARN_NOTE.to_string()]);
    }

    #[tokio::test]
    async fn trace_only_loop_keeps_running() {
        let mut config = config("api", "");
        config.ship_every = Duration::from_millis(10);
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "traced", LogLevel::Info));
        let handle = shipper.spawn_ship_loop(ShutdownReceiver::new());
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(!handle.is_finished());
        handle.abort();
        assert_eq!(shipper.buffered_len(), 0);
    }

    #[tokio::test]
    async fn shutdown_flushes_every_chunk_and_ends_the_loop() {
        let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
        let shipper = LogShipper::new(config("api", endpoint)).unwrap();
        let shutdown = ShutdownReceiver::new();
        let handle = shipper.spawn_ship_loop(shutdown.clone());
        tokio::time::sleep(Duration::from_millis(20)).await;
        for index in 0..SHIP_CHUNK + 1 {
            shipper.record(event("", index.to_string(), LogLevel::Info));
        }
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
    async fn shutdown_flush_stops_when_the_log_server_fails() {
        let (endpoint, _rx) = capture_server(StatusAction::Unavailable).await;
        let shipper = LogShipper::new(config("api", endpoint)).unwrap();
        shipper.record(event("", "keep", LogLevel::Info));
        let shutdown = ShutdownReceiver::new();
        shutdown.trigger();
        let handle = shipper.spawn_ship_loop(shutdown);
        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(shipper.buffered_events()[0].message, "keep");
    }

    #[tokio::test]
    async fn failed_ship_does_not_repeat_the_capacity_warning() {
        let (endpoint, _rx) = capture_server(StatusAction::Unavailable).await;
        let mut config = config("api", endpoint);
        config.capacity = 5;
        let shipper = LogShipper::new(config).unwrap();
        for message in ["a", "b", "c", "d"] {
            shipper.record(event("", message, LogLevel::Info));
        }
        shipper.ship_batch().await;
        let events = shipper.buffered_events();
        assert_eq!(
            messages_of(&events),
            ["a", "b", "c", "d", CAPACITY_WARN_NOTE]
        );
        assert_eq!(events[0].level, LogLevel::Info);
        assert_capacity_warn(&events[4]);
        shipper.record(event("", "e", LogLevel::Info));
        let events = shipper.buffered_events();
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
        let mut config = config("api", endpoint);
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
        let events = shipper.buffered_events();
        assert_eq!(messages_of(&events), ["e", "f", "g"]);
        assert_eq!(capacity_warn_count(&events), 0);
        shipper.record(event("", "h", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(
            messages_of(&events),
            ["e", "f", "g", "h", CAPACITY_WARN_NOTE]
        );
        assert_eq!(events[0].level, LogLevel::Info);
        assert_capacity_warn(&events[4]);
        shipper.record(event("", "i", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(
            messages_of(&events),
            ["f", "g", "h", CAPACITY_WARN_NOTE, "i"]
        );
        assert_eq!(capacity_warn_count(&events), 1);
    }

    #[tokio::test]
    async fn successful_ship_at_80_percent_does_not_warn_again() {
        let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
        let mut config = config("api", endpoint);
        config.capacity = 5_000;
        let shipper = LogShipper::new(config).unwrap();
        for index in 0..4_000 {
            shipper.record(event("", index.to_string(), LogLevel::Info));
        }
        let events = shipper.buffered_events();
        assert_eq!(events.len(), 4_001);
        assert_eq!(events[0].message, "0");
        assert_eq!(events[0].level, LogLevel::Info);
        assert_eq!(capacity_warn_count(&events), 1);
        assert_capacity_warn(events.last().unwrap());
        for index in 4_000..4_999 {
            shipper.record(event("", index.to_string(), LogLevel::Info));
        }
        assert_eq!(shipper.buffered_len(), 5_000);
        assert_eq!(capacity_warn_count(&shipper.buffered_events()), 1);
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 4_000);
        let posted = parse_body(&rx.recv().await.unwrap().body);
        assert_eq!(posted.len(), SHIP_CHUNK);
        assert_eq!(posted[0].message, "0");
        assert_eq!(posted[0].level, LogLevel::Info);
        assert_eq!(capacity_warn_count(&posted), 0);
        let left = shipper.buffered_events();
        assert_eq!(left[0].message, "1000");
        assert_eq!(left[0].level, LogLevel::Info);
        assert_eq!(capacity_warn_count(&left), 1);
        shipper.record(event("", "more", LogLevel::Info));
        let left = shipper.buffered_events();
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
        assert_eq!(capacity_warn_count(&shipper.buffered_events()), 1);
        for index in 0..998 {
            shipper.record(event("", format!("extra-{index}"), LogLevel::Info));
        }
        assert_eq!(shipper.buffered_len(), 3_999);
        assert_eq!(capacity_warn_count(&shipper.buffered_events()), 1);
        shipper.record(event("", "cross", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(events.len(), 4_001);
        assert_eq!(capacity_warn_count(&events), 2);
        assert_eq!(events[events.len() - 2].message, "cross");
        assert_eq!(events[events.len() - 2].level, LogLevel::Info);
        assert_capacity_warn(events.last().unwrap());
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
