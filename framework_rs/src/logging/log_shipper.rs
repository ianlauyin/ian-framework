use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::shutdown::ShutdownReceiver;

use super::{LogEvent, LogLevel, LogValue, ShipperConfig, SHIP_CHUNK};

pub(crate) const SHIP_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const CAPACITY_WARN_NOTE: &str = "SHIPPER/capacityMeets80%";
const MASK: &str = "***";

#[derive(Clone)]
pub struct LogShipper {
    pub(crate) inner: Arc<Inner>,
}

impl LogShipper {
    pub fn new(config: ShipperConfig) -> Option<Self> {
        if !is_valid_app(&config) {
            return None;
        }
        let endpoint = normalized_endpoint(config.endpoint.as_deref())?;
        let post_api_key = bearer_header(config.post_api_key.as_deref())?;
        let masks = config
            .mask_patterns
            .iter()
            .map(|pattern| Mask::new(pattern))
            .collect::<Option<Vec<_>>>()?;
        let http = endpoint.as_ref().map(|_| ship_client()).transpose().ok()?;
        log_mode(&config.app, endpoint.is_some());
        Some(Self {
            inner: Arc::new(Inner {
                app: config.app,
                endpoint,
                post_api_key,
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

    pub(crate) fn lock_buffer(&self) -> std::sync::MutexGuard<'_, Buffer> {
        self.inner
            .buffer
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    pub(crate) fn buffered_len(&self) -> usize {
        self.lock_buffer().events.len()
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

    pub(crate) async fn ship_batch(&self) {
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
        let posted = match serde_json::to_string(&batch) {
            Ok(body) => {
                let mut request = client
                    .post(&url)
                    .timeout(SHIP_TIMEOUT)
                    .header(reqwest::header::CONTENT_TYPE, "application/json");
                if let Some(post_api_key) = &self.inner.post_api_key {
                    request = request.header(reqwest::header::AUTHORIZATION, post_api_key.clone());
                }
                match request.body(body).send().await {
                    Ok(response) => response.status().is_success(),
                    Err(_) => false,
                }
            }
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
            .field(
                "post_api_key",
                &self.inner.post_api_key.as_ref().map(|_| MASK),
            )
            .field("buffered", &self.buffered_len())
            .finish_non_exhaustive()
    }
}

pub(crate) struct Buffer {
    pub(crate) events: VecDeque<LogEvent>,
    capacity_warned: bool,
}

pub(crate) struct Inner {
    pub(crate) app: String,
    pub(crate) endpoint: Option<String>,
    pub(crate) post_api_key: Option<reqwest::header::HeaderValue>,
    capacity: usize,
    ship_every: Duration,
    pub(crate) masks: Vec<Mask>,
    buffer: Mutex<Buffer>,
    pub(crate) http: Option<reqwest::Client>,
    shipping: tokio::sync::Mutex<()>,
}

fn is_valid_app(config: &ShipperConfig) -> bool {
    let mut chars = config.app.chars();
    let starts_alphanumeric = chars.next().is_some_and(|ch| ch.is_ascii_alphanumeric());
    starts_alphanumeric
        && config
            .app
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
        && config.capacity != 0
        && !config.ship_every.is_zero()
}

// The outer None is an endpoint that is not valid. The inner None is no endpoint.
fn normalized_endpoint(raw: Option<&str>) -> Option<Option<String>> {
    let Some(trimmed) = raw.map(str::trim).filter(|text| !text.is_empty()) else {
        return Some(None);
    };
    if trimmed.chars().any(char::is_whitespace) {
        return None;
    }
    let parsed = reqwest::Url::parse(trimmed).ok()?;
    if !is_plain_endpoint(&parsed) {
        return None;
    }
    Some(Some(parsed.to_string().trim_end_matches('/').to_string()))
}

fn is_plain_endpoint(url: &reqwest::Url) -> bool {
    let allowed =
        url.scheme() == "https" || (url.scheme() == "http" && http_host_allowed(url.host_str()));
    allowed
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

// The outer None is a key that is not valid. The inner None is no key.
fn bearer_header(raw: Option<&str>) -> Option<Option<reqwest::header::HeaderValue>> {
    let Some(trimmed) = raw.map(str::trim).filter(|text| !text.is_empty()) else {
        return Some(None);
    };
    if trimmed.chars().any(char::is_whitespace) {
        return None;
    }
    let mut value = reqwest::header::HeaderValue::from_str(&format!("Bearer {trimmed}")).ok()?;
    value.set_sensitive(true);
    Some(Some(value))
}

fn ship_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .connect_timeout(SHIP_TIMEOUT)
        .timeout(SHIP_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("ian_framework/", env!("CARGO_PKG_VERSION")))
        .build()
}

fn log_mode(app: &str, posts: bool) {
    if posts {
        tracing::info!(app = %app, "log shipping posts to the log server");
    } else {
        tracing::info!(app = %app, "log shipping is trace-only");
    }
}

// Each request carries the post API key. Plain http stays on this machine or on a
// private IPv4 address, which is another machine on the private network.
fn http_host_allowed(host: Option<&str>) -> bool {
    let Some(host) = host else {
        return false;
    };
    if matches!(host, "localhost" | "127.0.0.1" | "[::1]") {
        return true;
    }
    host.parse::<std::net::Ipv4Addr>()
        .is_ok_and(|ip| ip.is_private())
}

fn meets_capacity_warn(len: usize, capacity: usize) -> bool {
    (len as u128) * 5 >= (capacity as u128) * 4
}

// `pair` matches a `name=value` pair whose name holds a match of the pattern, so the value is masked too.
pub(crate) struct Mask {
    plain: regex::Regex,
    pair: regex::Regex,
    json_pair: regex::Regex,
}

impl Mask {
    pub(crate) fn new(pattern: &str) -> Option<Self> {
        let plain = regex::Regex::new(pattern).ok()?;
        let pair = regex::Regex::new(&format!(r"[\w.\-]*(?:{pattern})[\w.\-]*=[^&\s;]*")).ok()?;
        let json_pair = regex::Regex::new(&format!(
            r#""[^"]*(?:{pattern})[^"]*"\s*:\s*(?:"(?:[^"\\]|\\.)*"|[\w.\-]+)"#
        ))
        .ok()?;
        Some(Self {
            plain,
            pair,
            json_pair,
        })
    }
}

fn mask_event(masks: &[Mask], event: &mut LogEvent) {
    if masks.is_empty() {
        return;
    }
    if let Some(text) = event.error_message.as_mut() {
        mask_text(masks, text);
    }
    for (key, value) in event.context.iter_mut() {
        mask_entry(masks, key, value);
    }
    for (key, value) in event.extra.iter_mut() {
        mask_entry(masks, key, value);
    }
}

fn mask_entry(masks: &[Mask], key: &str, value: &mut LogValue) {
    if masks.iter().any(|mask| mask.plain.is_match(key)) {
        *value = LogValue::String(MASK.to_string());
    } else if let LogValue::String(text) = value {
        mask_text(masks, text);
    }
}

pub(crate) fn mask_text(masks: &[Mask], text: &mut String) {
    for mask in masks {
        if let std::borrow::Cow::Owned(masked) = mask.json_pair.replace_all(text, MASK) {
            *text = masked;
        }
    }
    for mask in masks {
        if let std::borrow::Cow::Owned(masked) = mask.pair.replace_all(text, MASK) {
            *text = masked;
        }
    }
    for mask in masks {
        if let std::borrow::Cow::Owned(masked) = mask.plain.replace_all(text, MASK) {
            *text = masked;
        }
    }
}

fn capacity_warn_event() -> LogEvent {
    LogEvent::warn(CAPACITY_WARN_NOTE)
}

fn trace_event(app: &str, event: &LogEvent) {
    let context = serde_json::to_string(&event.context).unwrap_or_default();
    let extra = serde_json::to_string(&event.extra).unwrap_or_default();
    macro_rules! emit {
        ($level:ident) => {
            tracing::$level!(
                app,
                id = %event.id,
                action = %event.action,
                latency_ms = ?event.latency_ms,
                error_message = ?event.error_message,
                context = %context,
                extra = %extra
            )
        };
    }
    match event.level {
        LogLevel::Info => emit!(info),
        LogLevel::Warn => emit!(warn),
        LogLevel::Error => emit!(error),
    }
}
