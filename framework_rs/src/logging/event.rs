use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use uuid::Uuid;

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
    pub(crate) latency_ms: u64,
    pub(crate) level: LogLevel,
    #[serde(default)]
    pub(crate) error_message: Option<String>,
    pub context: BTreeMap<String, LogValue>,
    pub extra: BTreeMap<String, LogValue>,
}

pub(crate) fn new_id() -> String {
    Uuid::now_v7().to_string()
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

    pub fn latency_ms(&self) -> u64 {
        self.latency_ms
    }

    pub fn level(&self) -> LogLevel {
        self.level
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub(crate) fn build(
        action: String,
        level: LogLevel,
        error_message: Option<String>,
    ) -> LogEvent {
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
