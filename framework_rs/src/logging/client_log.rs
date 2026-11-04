use std::collections::BTreeMap;

use axum::body::to_bytes;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::from_fn;
use axum::routing::post;
use axum::Router;
use chrono::{DateTime, Duration, FixedOffset};

use crate::http::middleware::{client_ip, require_same_origin, user_agent};
use crate::time::hkt_now;

use super::log_shipper::SHIP_CHUNK;
use super::{LogEvent, LogLevel, LogShipper, LogValue};

const CLIENT_LOG_PATH: &str = "/log";
const CLIENT_ACTION_MAX_CHARS: usize = 128;
const CLIENT_ERROR_MESSAGE_MAX_CHARS: usize = 1024;
const CLIENT_EXTRA_MAX_KEYS: usize = 32;
const CLIENT_EXTRA_VALUE_MAX_BYTES: usize = 2048;
const CLIENT_TS_MAX_AGE_HOURS: i64 = 24;
const CLIENT_ERROR_PLACEHOLDER: &str = "no error message";
pub(crate) const BODY_LIMIT: usize = 1024 * 1024;

pub fn client_events(events: &[LogEvent]) -> Option<Vec<LogEvent>> {
    if events.is_empty() || events.len() > SHIP_CHUNK {
        return None;
    }
    Some(events.iter().map(LogEvent::from_client).collect())
}

pub fn client_log_router(shipper: LogShipper) -> Router {
    Router::new()
        .route(
            CLIENT_LOG_PATH,
            post(receive).route_layer(from_fn(require_same_origin)),
        )
        .with_state(shipper)
}

impl LogEvent {
    pub fn from_client(&self) -> LogEvent {
        let now = hkt_now();
        let error_message = match self.level {
            LogLevel::Info => None,
            LogLevel::Warn => self
                .error_message
                .as_deref()
                .map(|text| client_text(text, CLIENT_ERROR_MESSAGE_MAX_CHARS))
                .filter(|text| !text.trim().is_empty()),
            LogLevel::Error => Some(client_error_message(self.error_message.as_deref())),
        };
        LogEvent {
            ts: client_ts(self.ts, now),
            id: String::new(),
            action: client_text(&self.action, CLIENT_ACTION_MAX_CHARS),
            latency_ms: None,
            level: self.level,
            error_message,
            context: BTreeMap::new(),
            extra: client_extra(&self.extra),
        }
    }
}

async fn receive(State(shipper): State<LogShipper>, request: Request) -> StatusCode {
    let ip = client_ip(&request);
    let user_agent = user_agent(request.headers());
    let Ok(body) = to_bytes(request.into_body(), BODY_LIMIT).await else {
        return StatusCode::PAYLOAD_TOO_LARGE;
    };
    let Ok(events) = serde_json::from_slice::<Vec<LogEvent>>(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    let Some(events) = client_events(&events) else {
        return StatusCode::BAD_REQUEST;
    };
    for mut event in events {
        event.context.insert("ip".to_string(), ip.clone().into());
        event
            .context
            .insert("user_agent".to_string(), user_agent.clone().into());
        shipper.record(event);
    }
    StatusCode::NO_CONTENT
}

fn client_ts(ts: DateTime<FixedOffset>, now: DateTime<FixedOffset>) -> DateTime<FixedOffset> {
    if ts > now || now - ts > Duration::hours(CLIENT_TS_MAX_AGE_HOURS) {
        return now;
    }
    ts.with_timezone(now.offset())
}

// The text is shown in log lines, so a client cannot choose a line break or a text above the limit.
fn client_text(text: &str, max_chars: usize) -> String {
    text.chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .take(max_chars)
        .collect()
}

fn client_error_message(message: Option<&str>) -> String {
    let text = client_text(message.unwrap_or_default(), CLIENT_ERROR_MESSAGE_MAX_CHARS);
    if text.trim().is_empty() {
        return CLIENT_ERROR_PLACEHOLDER.to_string();
    }
    text
}

fn client_extra(extra: &BTreeMap<String, LogValue>) -> BTreeMap<String, LogValue> {
    let mut kept = BTreeMap::new();
    for (key, value) in extra {
        if kept.len() >= CLIENT_EXTRA_MAX_KEYS {
            break;
        }
        if !extra_key_ok(key) {
            continue;
        }
        let Some(text) = extra_value(value) else {
            continue;
        };
        kept.insert(key.clone(), LogValue::String(text));
    }
    kept
}

fn extra_key_ok(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

fn extra_value(value: &LogValue) -> Option<String> {
    let text = match value {
        LogValue::String(text) => text.clone(),
        LogValue::Number(number) => number.to_string(),
        LogValue::Bool(flag) => flag.to_string(),
    };
    if text.len() > CLIENT_EXTRA_VALUE_MAX_BYTES || text.contains('\r') || text.contains('\n') {
        return None;
    }
    Some(text)
}
