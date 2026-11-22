use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::Number;
use uuid::Uuid;

use crate::exception::Exception;
use crate::time::hkt_now;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

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

impl PartialEq<serde_json::Value> for LogValue {
    fn eq(&self, other: &serde_json::Value) -> bool {
        serde_json::to_value(self).is_ok_and(|value| &value == other)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEvent {
    pub ts: DateTime<FixedOffset>,
    pub id: String,
    pub action: String,
    pub latency_ms: u64,
    pub level: LogLevel,
    #[serde(default)]
    pub error_message: Option<String>,
    pub context: BTreeMap<String, LogValue>,
    pub extra: BTreeMap<String, LogValue>,
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

    pub fn warn_from(action: impl Into<String>, exception: &Exception) -> LogEvent {
        Self::build(
            action.into(),
            LogLevel::Warn,
            Some(exception_text(exception)),
        )
    }

    pub fn error_from(action: impl Into<String>, exception: &Exception) -> LogEvent {
        Self::build(
            action.into(),
            LogLevel::Error,
            Some(exception_text(exception)),
        )
    }

    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<LogValue>) -> LogEvent {
        self.context.insert(key.into(), value.into());
        self
    }

    pub fn with_extra(mut self, key: impl Into<String>, value: impl Into<LogValue>) -> LogEvent {
        self.extra.insert(key.into(), value.into());
        self
    }

    pub fn with_latency(mut self, elapsed: Duration) -> LogEvent {
        self.latency_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        self
    }

    pub fn build(action: String, level: LogLevel, error_message: Option<String>) -> LogEvent {
        LogEvent {
            ts: hkt_now(),
            id: new_id(),
            action,
            latency_ms: 0,
            level,
            error_message,
            context: BTreeMap::new(),
            extra: BTreeMap::new(),
        }
    }
}

fn new_id() -> String {
    Uuid::now_v7().to_string()
}

fn exception_text(exception: &Exception) -> String {
    format!("{exception}\n{}", String::from(exception))
}
