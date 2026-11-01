use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};

use crate::time::hkt_now;

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
    pub ts: DateTime<FixedOffset>,
    pub id: String,
    pub action: String,
    pub latency_ms: Option<u64>,
    pub level: LogLevel,
    pub context: BTreeMap<String, LogValue>,
    pub info: BTreeMap<String, LogValue>,
}

impl LogEvent {
    pub fn sanitize_untrusted_client(&self) -> LogEvent {
        LogEvent {
            ts: hkt_now(),
            id: String::new(),
            action: self.action.clone(),
            latency_ms: None,
            level: self.level,
            context: BTreeMap::new(),
            info: client_info(&self.info),
        }
    }
}

fn client_info(info: &BTreeMap<String, LogValue>) -> BTreeMap<String, LogValue> {
    let mut kept = BTreeMap::new();
    for (key, value) in info {
        if !info_key_ok(key) {
            continue;
        }
        let Some(text) = info_value(value) else {
            continue;
        };
        kept.insert(key.clone(), LogValue::String(text));
    }
    kept
}

fn info_key_ok(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

fn info_value(value: &LogValue) -> Option<String> {
    let text = match value {
        LogValue::String(text) => text.clone(),
        LogValue::Number(number) => number.to_string(),
        LogValue::Bool(flag) => flag.to_string(),
    };
    if text.contains('\r') || text.contains('\n') {
        return None;
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::event;
    use serde_json::json;

    #[test]
    fn level_text_matches_the_json_text_and_parses_back() {
        for level in [LogLevel::Info, LogLevel::Warn, LogLevel::Error] {
            assert_eq!(serde_json::to_value(level).unwrap(), json!(level.as_str()));
            assert_eq!(LogLevel::parse(level.as_str()), Some(level));
        }
        for text in ["", "INFO", "Info", "debug", " info"] {
            assert_eq!(LogLevel::parse(text), None, "{text:?}");
        }
    }

    fn sample() -> LogEvent {
        let mut event = event("GET /health", LogLevel::Info);
        event.ts = DateTime::parse_from_rfc3339("2026-10-18T20:00:00+08:00").unwrap();
        event.id = "req-1".to_string();
        event
            .context
            .insert("ip".to_string(), LogValue::from("203.0.113.8"));
        event
            .context
            .insert("user_agent".to_string(), LogValue::from("demo-agent"));
        event.latency_ms = Some(12);
        event
            .info
            .insert("region".to_string(), LogValue::from("asia"));
        event
    }

    #[test]
    fn json_object_has_every_schema_field() {
        let json = serde_json::to_string(&sample()).unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();
        for key in [
            "ts",
            "id",
            "action",
            "latency_ms",
            "level",
            "context",
            "info",
        ] {
            assert!(value.get(key).is_some(), "{key}");
        }
        assert!(value.get("app").is_none());
        assert!(value.get("project").is_none());
        assert_eq!(value["ts"], "2026-10-18T20:00:00+08:00");
        assert_eq!(value["id"], "req-1");
        assert!(value.get("ip").is_none());
        assert!(value.get("user_agent").is_none());
        assert_eq!(value["context"]["ip"], "203.0.113.8");
        assert_eq!(value["context"]["user_agent"], "demo-agent");
        assert_eq!(value["action"], "GET /health");
        assert!(value.get("request").is_none());
        assert!(value.get("request_id").is_none());
        assert!(value.get("method").is_none());
        assert!(value.get("path").is_none());
        assert_eq!(value["latency_ms"], 12);
        assert_eq!(value["level"], "info");
        assert_eq!(value["info"]["region"], "asia");
    }

    #[test]
    fn empty_http_fields_are_null_or_empty() {
        let json = serde_json::to_string(&event("", LogLevel::Warn)).unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();
        assert!(value.get("app").is_none());
        assert!(value.get("project").is_none());
        assert_eq!(value["id"], "");
        assert!(value["context"].as_object().unwrap().is_empty());
        assert_eq!(value["action"], "");
        assert!(value.get("request").is_none());
        assert!(value["latency_ms"].is_null());
        assert_eq!(value["level"], "warn");
        assert!(value["info"].as_object().unwrap().is_empty());
    }

    #[test]
    fn json_object_round_trips() {
        let event = sample();
        let parsed: LogEvent =
            serde_json::from_str(&serde_json::to_string(&event).unwrap()).unwrap();
        assert_eq!(parsed, event);
    }

    #[test]
    fn a_new_event_ts_has_the_hkt_offset() {
        let value = serde_json::to_value(event("", LogLevel::Info)).unwrap();
        assert!(value["ts"].as_str().unwrap().ends_with("+08:00"));
    }

    #[test]
    fn a_sanitized_event_ts_has_the_hkt_offset() {
        let stored = event("/files", LogLevel::Info).sanitize_untrusted_client();
        let value = serde_json::to_value(stored).unwrap();
        assert!(value["ts"].as_str().unwrap().ends_with("+08:00"));
    }

    #[test]
    fn a_utc_ts_is_accepted() {
        let mut value = serde_json::to_value(sample()).unwrap();
        value["ts"] = json!("2026-10-18T12:00:00Z");
        let parsed: LogEvent = serde_json::from_value(value).unwrap();
        assert_eq!(parsed.ts, sample().ts);
        assert_eq!(parsed.ts.offset().local_minus_utc(), 0);
    }

    #[test]
    fn a_non_scalar_context_or_info_value_is_rejected() {
        for field in ["context", "info"] {
            for bad in [json!(null), json!({"a": 1}), json!([1])] {
                let mut value = serde_json::to_value(sample()).unwrap();
                value[field]["bad"] = bad;
                assert!(serde_json::from_value::<LogEvent>(value).is_err());
            }
        }
    }

    #[test]
    fn a_missing_schema_field_is_rejected() {
        let json = r#"{"ts":"2026-10-18T12:00:00Z","action":"note"}"#;
        assert!(serde_json::from_str::<LogEvent>(json).is_err());
    }

    #[test]
    fn sanitize_untrusted_client_drops_untrusted_fields() {
        let mut event = event("GET /download?sig=abc#top", LogLevel::Warn);
        event.ts = DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z").unwrap();
        event.id = "client-id".to_string();
        event
            .context
            .insert("ip".to_string(), LogValue::from("203.0.113.8"));
        event.latency_ms = Some(42);
        let before = hkt_now();
        let stored = event.sanitize_untrusted_client();
        let after = hkt_now();
        assert_eq!(event.id, "client-id");
        assert_eq!(stored.id, "");
        assert!(stored.context.is_empty());
        assert_eq!(stored.latency_ms, None);
        assert_eq!(stored.level, LogLevel::Warn);
        assert_eq!(stored.action, "GET /download?sig=abc#top");
        assert_ne!(stored.ts, event.ts);
        assert!(stored.ts >= before && stored.ts <= after);
    }

    #[test]
    fn sanitize_untrusted_client_keeps_the_client_action() {
        for action in [
            "/api/foo?x=1",
            "/api/foo#part",
            "/api/foo?x=1#part",
            "/api/foo#part?x=1",
            "",
            "?x=1",
            "#part",
            "/",
            "GET /api/foo?token=secret",
            "plain",
        ] {
            let stored = event(action, LogLevel::Info).sanitize_untrusted_client();
            assert_eq!(stored.action, action);
        }
    }

    #[test]
    fn sanitize_untrusted_client_filters_info() {
        let mut client = event("/files", LogLevel::Info);
        client
            .info
            .insert("bad key".to_string(), LogValue::from("no"));
        client
            .info
            .insert("k01".to_string(), LogValue::from("kept"));
        client
            .info
            .insert("k02".to_string(), LogValue::from("a\nb"));
        client
            .info
            .insert("k03".to_string(), LogValue::from("a\rb"));
        client
            .info
            .insert("k07".to_string(), LogValue::from("x".repeat(250)));
        client
            .info
            .insert("k08".to_string(), LogValue::from("ninth"));
        client.info.insert("k09".to_string(), LogValue::from(true));
        let stored = client.sanitize_untrusted_client();
        assert_eq!(
            stored.info.keys().cloned().collect::<Vec<_>>(),
            vec![
                "k01".to_string(),
                "k07".to_string(),
                "k08".to_string(),
                "k09".to_string(),
            ]
        );
        assert_eq!(stored.info["k01"], json!("kept"));
        assert_eq!(stored.info["k07"], json!("x".repeat(250)));
        assert_eq!(stored.info["k08"], json!("ninth"));
        assert_eq!(stored.info["k09"], json!("true"));
        assert!(!stored.info.contains_key("bad key"));

        let mut scalars = event("/files", LogLevel::Error);
        scalars.info.insert("a/b".to_string(), LogValue::from("no"));
        scalars
            .info
            .insert("a_b-c.d".to_string(), LogValue::from("yes"));
        scalars
            .info
            .insert("a".repeat(65), LogValue::from("long-key"));
        scalars.info.insert("count".to_string(), LogValue::from(12));
        scalars
            .info
            .insert("flag".to_string(), LogValue::from(true));
        scalars
            .info
            .insert("off".to_string(), LogValue::from(false));
        scalars
            .info
            .insert("ratio".to_string(), LogValue::from(1.5));
        let stored = scalars.sanitize_untrusted_client();
        assert_eq!(stored.info.get("a/b"), None);
        assert_eq!(stored.info[&"a".repeat(65)], json!("long-key"));
        assert_eq!(stored.info["a_b-c.d"], json!("yes"));
        assert_eq!(stored.info["count"], json!("12"));
        assert_eq!(stored.info["flag"], json!("true"));
        assert_eq!(stored.info["off"], json!("false"));
        assert_eq!(stored.info["ratio"], json!("1.5"));

        let mut text = event("/files", LogLevel::Info);
        text.info.insert(String::new(), LogValue::from("no"));
        text.info.insert("café".to_string(), LogValue::from("no"));
        text.info.insert("ok!".to_string(), LogValue::from("no"));
        let long = "中".repeat(67);
        text.info
            .insert("text".to_string(), LogValue::from(long.clone()));
        let stored = text.sanitize_untrusted_client();
        assert_eq!(stored.info.get(""), None);
        assert_eq!(stored.info.get("café"), None);
        assert_eq!(stored.info.get("ok!"), None);
        assert_eq!(stored.info["text"], json!(long));

        let mut later = event("/files", LogLevel::Info);
        for index in 0..8 {
            later
                .info
                .insert(format!("bad {index}"), LogValue::from("no"));
        }
        later.info.insert("zzzz".to_string(), LogValue::from("yes"));
        let stored = later.sanitize_untrusted_client();
        assert_eq!(stored.info.len(), 1);
        assert_eq!(stored.info["zzzz"], json!("yes"));
    }
}
