use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Number, Value};

use crate::time::hkt_now;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
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
