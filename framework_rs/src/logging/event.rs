use std::collections::HashMap;

use chrono::{DateTime, Duration, FixedOffset};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEvent {
    pub(crate) ts: DateTime<FixedOffset>,
    pub(crate) id: String,
    pub(crate) action: String,
    pub(crate) latency_ms: Option<u64>,
    pub(crate) level: LogLevel,
    #[serde(default, deserialize_with = "text_or_none")]
    pub(crate) error_message: Option<String>,
    pub(crate) context: HashMap<String, String>,
    #[serde(deserialize_with = "flat_extra")]
    pub(crate) extra: HashMap<String, String>,
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

    pub fn context(&self) -> &HashMap<String, String> {
        &self.context
    }

    pub fn extra(&self) -> &HashMap<String, String> {
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
            context: HashMap::new(),
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
            context: HashMap::new(),
            extra: HashMap::new(),
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

fn client_extra(extra: &HashMap<String, String>) -> HashMap<String, String> {
    let mut keys: Vec<&String> = extra.keys().collect();
    keys.sort();
    let mut kept = HashMap::new();
    for key in keys {
        if kept.len() >= CLIENT_EXTRA_MAX_KEYS {
            break;
        }
        if !extra_key_ok(key) {
            continue;
        }
        let text = &extra[key];
        if !extra_value_ok(text) {
            continue;
        }
        kept.insert(key.clone(), text.clone());
    }
    kept
}

fn extra_key_ok(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

fn extra_value_ok(text: &str) -> bool {
    text.len() <= CLIENT_EXTRA_VALUE_MAX_BYTES && !text.contains('\r') && !text.contains('\n')
}

// `extra` has one level. A string, a number or a bool becomes text. An array, an object and null are dropped.
pub(crate) fn flatten_extra(fields: Map<String, Value>) -> HashMap<String, String> {
    fields
        .into_iter()
        .filter_map(|(key, value)| {
            let text = match value {
                Value::String(text) => text,
                Value::Number(number) => number.to_string(),
                Value::Bool(flag) => flag.to_string(),
                _ => return None,
            };
            Some((key, text))
        })
        .collect()
}

fn flat_extra<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<HashMap<String, String>, D::Error> {
    Ok(flatten_extra(Map::deserialize(deserializer)?))
}
