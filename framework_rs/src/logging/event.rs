use std::collections::BTreeMap;

use chrono::{DateTime, Duration, FixedOffset};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Number, Value};

use crate::time::hkt_now;

const CLIENT_ACTION_MAX_CHARS: usize = 128;
const CLIENT_ERROR_MESSAGE_MAX_CHARS: usize = 1024;
const CLIENT_EXTRA_MAX_KEYS: usize = 32;
const CLIENT_EXTRA_VALUE_MAX_BYTES: usize = 2048;
const CLIENT_TS_MAX_AGE_HOURS: i64 = 24;
const CLIENT_ERROR_PLACEHOLDER: &str = "no error message";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }

    pub fn parse(text: &str) -> Option<LogLevel> {
        match text {
            "info" => Some(LogLevel::Info),
            "warn" => Some(LogLevel::Warn),
            "error" => Some(LogLevel::Error),
            _ => None,
        }
    }
}

// An untagged enum, so deserialize rejects null, an array and an object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LogValue {
    Bool(bool),
    Number(Number),
    String(String),
}

impl From<&str> for LogValue {
    fn from(value: &str) -> Self {
        LogValue::String(value.to_string())
    }
}

impl From<String> for LogValue {
    fn from(value: String) -> Self {
        LogValue::String(value)
    }
}

impl From<bool> for LogValue {
    fn from(value: bool) -> Self {
        LogValue::Bool(value)
    }
}

impl From<u64> for LogValue {
    fn from(value: u64) -> Self {
        LogValue::Number(value.into())
    }
}

impl From<f64> for LogValue {
    fn from(value: f64) -> Self {
        match Number::from_f64(value) {
            Some(number) => LogValue::Number(number),
            None => LogValue::String(value.to_string()),
        }
    }
}

impl PartialEq<&str> for LogValue {
    fn eq(&self, other: &&str) -> bool {
        matches!(self, LogValue::String(text) if text == other)
    }
}

impl PartialEq<Value> for LogValue {
    fn eq(&self, other: &Value) -> bool {
        serde_json::to_value(self).is_ok_and(|value| &value == other)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEvent {
    pub(crate) ts: DateTime<FixedOffset>,
    pub(crate) id: String,
    pub(crate) action: String,
    pub(crate) latency_ms: Option<u64>,
    pub(crate) level: LogLevel,
    #[serde(default, deserialize_with = "text_or_none")]
    pub(crate) error_message: Option<String>,
    pub(crate) context: BTreeMap<String, LogValue>,
    pub(crate) extra: BTreeMap<String, LogValue>,
}

impl LogEvent {
    pub fn info(action: impl Into<String>) -> LogEvent {
        Self::build(action.into(), LogLevel::Info, None)
    }

    pub fn warn(action: impl Into<String>) -> LogEvent {
        Self::build(action.into(), LogLevel::Warn, None)
    }

    pub fn warn_with(action: impl Into<String>, error_message: impl Into<String>) -> LogEvent {
        Self::build(action.into(), LogLevel::Warn, Some(error_message.into()))
    }

    pub fn error(action: impl Into<String>, error_message: impl Into<String>) -> LogEvent {
        Self::build(action.into(), LogLevel::Error, Some(error_message.into()))
    }

    pub fn ts(&self) -> DateTime<FixedOffset> {
        self.ts
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    pub fn action(&self) -> &str {
        &self.action
    }

    pub fn latency_ms(&self) -> Option<u64> {
        self.latency_ms
    }

    pub fn level(&self) -> LogLevel {
        self.level
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub fn context(&self) -> &BTreeMap<String, LogValue> {
        &self.context
    }

    pub fn extra(&self) -> &BTreeMap<String, LogValue> {
        &self.extra
    }

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

    fn build(action: String, level: LogLevel, error_message: Option<String>) -> LogEvent {
        LogEvent {
            ts: hkt_now(),
            id: String::new(),
            action,
            latency_ms: None,
            level,
            error_message,
            context: BTreeMap::new(),
            extra: BTreeMap::new(),
        }
    }
}

// One bad `error_message` must not reject the batch, so a value that is not a string reads as none.
fn text_or_none<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::String(text) => Some(text),
        _ => None,
    })
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
