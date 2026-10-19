use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{LogError, LogErrorKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEvent {
    pub ts: DateTime<Utc>,
    pub app: String,
    pub project: String,
    pub request_id: String,
    pub method: String,
    pub path: String,
    pub status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub level: LogLevel,
    pub message: String,
    pub extra: BTreeMap<String, Value>,
}

pub fn event(app: impl Into<String>, level: LogLevel, message: impl Into<String>) -> LogEvent {
    LogEvent {
        ts: Utc::now(),
        app: app.into(),
        project: String::new(),
        request_id: String::new(),
        method: String::new(),
        path: String::new(),
        status: None,
        latency_ms: None,
        level,
        message: message.into(),
        extra: BTreeMap::new(),
    }
}

pub fn to_json_line(event: &LogEvent) -> Result<String, LogError> {
    serde_json::to_string(event).map_err(|err| {
        LogError::with_source(LogErrorKind::Encode, "cannot encode a log event", err)
    })
}

pub fn parse_json_line(line: &str) -> Result<LogEvent, LogError> {
    serde_json::from_str(line)
        .map_err(|err| LogError::with_source(LogErrorKind::Encode, "cannot parse a log event", err))
}

pub fn to_jsonl(events: &[LogEvent]) -> Result<String, LogError> {
    let mut body = String::new();
    for event in events {
        body.push_str(&to_json_line(event)?);
        body.push('\n');
    }
    Ok(body)
}

pub fn parse_jsonl(body: &str) -> Result<Vec<LogEvent>, LogError> {
    let mut events = Vec::new();
    for line in body.lines() {
        if line.trim().is_empty() {
            continue;
        }
        events.push(parse_json_line(line.trim())?);
    }
    Ok(events)
}

pub(super) fn level_rank(level: LogLevel) -> u8 {
    match level {
        LogLevel::Debug => 0,
        LogLevel::Info => 1,
        LogLevel::Warn => 2,
        LogLevel::Error => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> LogEvent {
        let mut event = event("cs", LogLevel::Info, "request");
        event.ts = DateTime::parse_from_rfc3339("2026-10-18T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        event.project = "demo".to_string();
        event.request_id = "req-1".to_string();
        event.method = "GET".to_string();
        event.path = "/health".to_string();
        event.status = Some(204);
        event.latency_ms = Some(12);
        event.extra.insert("region".to_string(), json!("asia"));
        event
    }

    #[test]
    fn json_line_has_every_schema_field() {
        let line = to_json_line(&sample()).unwrap();
        let value: Value = serde_json::from_str(&line).unwrap();
        for key in [
            "ts",
            "app",
            "project",
            "request_id",
            "method",
            "path",
            "status",
            "latency_ms",
            "level",
            "message",
            "extra",
        ] {
            assert!(value.get(key).is_some(), "{key}");
        }
        assert!(value["ts"]
            .as_str()
            .unwrap()
            .contains("2026-10-18T12:00:00"));
        assert_eq!(value["app"], "cs");
        assert_eq!(value["project"], "demo");
        assert_eq!(value["request_id"], "req-1");
        assert_eq!(value["method"], "GET");
        assert_eq!(value["path"], "/health");
        assert_eq!(value["status"], 204);
        assert_eq!(value["latency_ms"], 12);
        assert_eq!(value["level"], "info");
        assert_eq!(value["message"], "request");
        assert_eq!(value["extra"]["region"], "asia");
        let parsed = DateTime::parse_from_rfc3339(value["ts"].as_str().unwrap()).unwrap();
        assert_eq!(parsed.offset().local_minus_utc(), 0);
    }

    #[test]
    fn empty_http_fields_are_null_or_empty() {
        let line = to_json_line(&event("cs", LogLevel::Warn, "note")).unwrap();
        let value: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value["app"], "cs");
        assert_eq!(value["project"], "");
        assert_eq!(value["request_id"], "");
        assert_eq!(value["method"], "");
        assert_eq!(value["path"], "");
        assert!(value["status"].is_null());
        assert!(value["latency_ms"].is_null());
        assert_eq!(value["level"], "warn");
        assert!(value["extra"].as_object().unwrap().is_empty());
    }

    #[test]
    fn json_line_round_trips() {
        let event = sample();
        let parsed = parse_json_line(&to_json_line(&event).unwrap()).unwrap();
        assert_eq!(parsed, event);
    }

    #[test]
    fn jsonl_round_trips_and_skips_blank_lines() {
        let events = vec![sample(), event("cs", LogLevel::Error, "down")];
        let body = format!("\n{}\n", to_jsonl(&events).unwrap());
        let parsed = parse_jsonl(&body).unwrap();
        assert_eq!(parsed, events);
    }

    #[test]
    fn a_missing_schema_field_is_rejected() {
        let line = r#"{"ts":"2026-10-18T12:00:00Z","project":"demo"}"#;
        let err = parse_json_line(line).unwrap_err();
        assert_eq!(err.kind(), LogErrorKind::Encode);
    }
}
