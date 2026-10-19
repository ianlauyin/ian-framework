use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    pub id: String,
    pub ip: String,
    pub user_agent: String,
    pub action: String,
    pub status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub level: LogLevel,
    pub message: String,
    pub extra: BTreeMap<String, Value>,
}

pub fn event(action: impl Into<String>, message: impl Into<String>, level: LogLevel) -> LogEvent {
    LogEvent {
        ts: Utc::now(),
        id: String::new(),
        ip: String::new(),
        user_agent: String::new(),
        action: action.into(),
        status: None,
        latency_ms: None,
        level,
        message: message.into(),
        extra: BTreeMap::new(),
    }
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
        let mut event = event("GET /health", "request", LogLevel::Info);
        event.ts = DateTime::parse_from_rfc3339("2026-10-18T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        event.id = "req-1".to_string();
        event.ip = "203.0.113.8".to_string();
        event.user_agent = "demo-agent".to_string();
        event.status = Some(204);
        event.latency_ms = Some(12);
        event.extra.insert("region".to_string(), json!("asia"));
        event
    }

    #[test]
    fn json_object_has_every_schema_field() {
        let json = serde_json::to_string(&sample()).unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();
        for key in [
            "ts",
            "id",
            "ip",
            "user_agent",
            "action",
            "status",
            "latency_ms",
            "level",
            "message",
            "extra",
        ] {
            assert!(value.get(key).is_some(), "{key}");
        }
        assert!(value.get("app").is_none());
        assert!(value.get("project").is_none());
        assert!(value["ts"]
            .as_str()
            .unwrap()
            .contains("2026-10-18T12:00:00"));
        assert_eq!(value["id"], "req-1");
        assert_eq!(value["ip"], "203.0.113.8");
        assert_eq!(value["user_agent"], "demo-agent");
        assert_eq!(value["action"], "GET /health");
        assert!(value.get("request").is_none());
        assert!(value.get("request_id").is_none());
        assert!(value.get("method").is_none());
        assert!(value.get("path").is_none());
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
        let json = serde_json::to_string(&event("", "note", LogLevel::Warn)).unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();
        assert!(value.get("app").is_none());
        assert!(value.get("project").is_none());
        assert_eq!(value["id"], "");
        assert_eq!(value["ip"], "");
        assert_eq!(value["user_agent"], "");
        assert_eq!(value["action"], "");
        assert!(value.get("request").is_none());
        assert!(value["status"].is_null());
        assert!(value["latency_ms"].is_null());
        assert_eq!(value["level"], "warn");
        assert!(value["extra"].as_object().unwrap().is_empty());
    }

    #[test]
    fn json_object_round_trips() {
        let event = sample();
        let parsed: LogEvent =
            serde_json::from_str(&serde_json::to_string(&event).unwrap()).unwrap();
        assert_eq!(parsed, event);
    }

    #[test]
    fn request_json_round_trips() {
        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct RequestJson {
            events: Vec<LogEvent>,
        }

        let events = vec![sample(), event("", "down", LogLevel::Error)];
        let body = serde_json::to_string(&RequestJson {
            events: events.clone(),
        })
        .unwrap();
        let parsed: RequestJson = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed.events, events);
        let value: Value = serde_json::from_str(&body).unwrap();
        assert!(value.as_object().is_some());
        assert!(value["events"].is_array());
        assert!(serde_json::from_str::<Vec<LogEvent>>(&body).is_err());
    }

    #[test]
    fn a_missing_schema_field_is_rejected() {
        let json = r#"{"ts":"2026-10-18T12:00:00Z","message":"note"}"#;
        assert!(serde_json::from_str::<LogEvent>(json).is_err());
    }
}
