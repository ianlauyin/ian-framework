use std::collections::BTreeMap;

use axum::http::HeaderMap;
use chrono::{DateTime, Duration, FixedOffset};
use serde::{Deserialize, Serialize};

use crate::http::headers::{client_ip, user_agent};
use crate::logging::event::new_id;
use crate::logging::log_shipper::SHIP_CHUNK;
use crate::logging::{LogEvent, LogLevel, LogValue};
use crate::time::hkt_now;

const CLIENT_ACTION_MAX_CHARS: usize = 128;
const CLIENT_ERROR_MESSAGE_MAX_CHARS: usize = 1024;
const CLIENT_EXTRA_MAX_KEYS: usize = 32;
const CLIENT_EXTRA_VALUE_MAX_BYTES: usize = 2048;
const CLIENT_TS_MAX_AGE_HOURS: i64 = 24;
const CLIENT_ERROR_PLACEHOLDER: &str = "unknown error";

#[derive(Debug, Deserialize)]
pub struct ClientLog {
    pub events: Vec<ClientLogEvent>,
}

impl ClientLog {
    pub fn log_events(&self, headers: &HeaderMap) -> Option<Vec<LogEvent>> {
        if self.events.is_empty() || self.events.len() > SHIP_CHUNK {
            return None;
        }
        let ip = client_ip(headers);
        let user_agent = user_agent(headers);
        Some(
            self.events
                .iter()
                .map(|event| client_event(event, &ip, &user_agent))
                .collect(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientLogEvent {
    pub(crate) ts: DateTime<FixedOffset>,
    pub(crate) action: String,
    pub(crate) latency_ms: Option<u64>,
    pub(crate) level: LogLevel,
    #[serde(default)]
    pub(crate) error_message: Option<String>,
    pub(crate) context: BTreeMap<String, LogValue>,
    pub(crate) extra: BTreeMap<String, LogValue>,
}

fn client_event(event: &ClientLogEvent, ip: &str, user_agent: &str) -> LogEvent {
    let now = hkt_now();
    let mut context = client_fields(&event.context);
    context.insert("ip".to_string(), ip.into());
    context.insert("user_agent".to_string(), user_agent.into());
    LogEvent {
        ts: client_ts(event.ts, now),
        id: new_id(),
        action: client_text(&event.action, CLIENT_ACTION_MAX_CHARS),
        latency_ms: event.latency_ms,
        level: event.level,
        error_message: client_error_message(event),
        context,
        extra: client_fields(&event.extra),
    }
}

fn client_error_message(event: &ClientLogEvent) -> Option<String> {
    match event.level {
        LogLevel::Info => None,
        LogLevel::Warn => event
            .error_message
            .as_deref()
            .map(|text| client_text(text, CLIENT_ERROR_MESSAGE_MAX_CHARS))
            .filter(|text| !text.trim().is_empty()),
        LogLevel::Error => {
            let text = client_text(
                event.error_message.as_deref().unwrap_or_default(),
                CLIENT_ERROR_MESSAGE_MAX_CHARS,
            );
            Some(if text.trim().is_empty() {
                CLIENT_ERROR_PLACEHOLDER.to_string()
            } else {
                text
            })
        }
    }
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

fn client_fields(extra: &BTreeMap<String, LogValue>) -> BTreeMap<String, LogValue> {
    let mut kept = BTreeMap::new();
    for (key, value) in extra {
        if kept.len() >= CLIENT_EXTRA_MAX_KEYS {
            break;
        }
        if !extra_key_ok(key) {
            continue;
        }
        if !extra_value_ok(value) {
            continue;
        }
        kept.insert(key.clone(), value.clone());
    }
    kept
}

fn extra_key_ok(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

fn extra_value_ok(value: &LogValue) -> bool {
    match value {
        LogValue::String(text) => {
            text.len() <= CLIENT_EXTRA_VALUE_MAX_BYTES && !text.contains(['\r', '\n'])
        }
        LogValue::Number(_) | LogValue::Bool(_) => true,
    }
}
