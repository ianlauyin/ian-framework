use std::collections::BTreeMap;

use chrono::{DateTime, Duration, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

use crate::time::hkt_now;

const CLIENT_ACTION_MAX_CHARS: usize = 128;
const CLIENT_INFO_MAX_KEYS: usize = 32;
const CLIENT_INFO_VALUE_MAX_BYTES: usize = 2048;
const CLIENT_TS_MAX_AGE_HOURS: i64 = 24;

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
    pub(crate) ts: DateTime<FixedOffset>,
    pub(crate) id: String,
    pub(crate) action: String,
    pub(crate) latency_ms: Option<u64>,
    pub(crate) level: LogLevel,
    pub(crate) context: BTreeMap<String, LogValue>,
    pub(crate) info: Map<String, Value>,
}

impl LogEvent {
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

    pub fn context(&self) -> &BTreeMap<String, LogValue> {
        &self.context
    }

    pub fn info(&self) -> &Map<String, Value> {
        &self.info
    }

    pub fn new(&self) -> LogEvent {
        let now = hkt_now();
        LogEvent {
            ts: client_ts(self.ts, now),
            id: String::new(),
            action: client_action(&self.action),
            latency_ms: None,
            level: self.level,
            context: BTreeMap::new(),
            info: client_info(&self.info),
        }
    }
}

fn client_ts(ts: DateTime<FixedOffset>, now: DateTime<FixedOffset>) -> DateTime<FixedOffset> {
    if ts > now || now - ts > Duration::hours(CLIENT_TS_MAX_AGE_HOURS) {
        return now;
    }
    ts.with_timezone(now.offset())
}

// The action is shown in log lines, so a client cannot choose a line break or a long text.
fn client_action(action: &str) -> String {
    action
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .take(CLIENT_ACTION_MAX_CHARS)
        .collect()
}

fn client_info(info: &Map<String, Value>) -> Map<String, Value> {
    let mut kept = Map::new();
    for (key, value) in info {
        if kept.len() >= CLIENT_INFO_MAX_KEYS {
            break;
        }
        if !info_key_ok(key) {
            continue;
        }
        let Some(text) = info_value(value) else {
            continue;
        };
        kept.insert(key.clone(), Value::String(text));
    }
    kept
}

fn info_key_ok(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

fn info_value(value: &Value) -> Option<String> {
    let text = match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        _ => return None,
    };
    if text.len() > CLIENT_INFO_VALUE_MAX_BYTES || text.contains('\r') || text.contains('\n') {
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
    fn client_action_and_info_are_bounded() {
        let batch_event = |action: &str, info: Value| -> LogEvent {
            let mut client = event(action, LogLevel::Info);
            client.info = info.as_object().cloned().unwrap_or_default();
            client.new()
        };
        let forged = batch_event("a\nINFO forged\r", json!({}));
        assert!(!forged.action().contains(['\n', '\r']));
        assert_eq!(batch_event(&"x".repeat(500), json!({})).action().len(), 128);
        let long = "y".repeat(2049);
        assert!(batch_event("a", json!({ "k": long })).info().is_empty());
        let many: Map<String, Value> = (0..100).map(|n| (format!("k{n}"), json!("v"))).collect();
        assert_eq!(batch_event("a", Value::Object(many)).info().len(), 32);
    }

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
        event.info.insert("region".to_string(), json!("asia"));
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
    fn getters_and_set_id_expose_the_fields() {
        let mut event = sample();
        event.set_id("req-2".to_string());
        assert_eq!(event.ts(), sample().ts);
        assert_eq!(event.id(), "req-2");
        assert_eq!(event.action(), "GET /health");
        assert_eq!(event.latency_ms(), Some(12));
        assert_eq!(event.level(), LogLevel::Info);
        assert_eq!(event.context()["ip"], "203.0.113.8");
        assert_eq!(event.info()["region"], "asia");
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
        let stored = event("/files", LogLevel::Info).new();
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
    fn a_non_scalar_context_value_is_rejected() {
        for field in ["context"] {
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
    fn new_drops_untrusted_fields() {
        let mut event = event("GET /download?sig=abc#top", LogLevel::Warn);
        event.ts = DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z").unwrap();
        event.id = "client-id".to_string();
        event
            .context
            .insert("ip".to_string(), LogValue::from("203.0.113.8"));
        event.latency_ms = Some(42);
        let before = hkt_now();
        let stored = event.new();
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
    fn new_keeps_a_recent_client_ts_in_hkt() {
        let mut event = event("note", LogLevel::Info);
        let ts = hkt_now() - Duration::minutes(5);
        event.ts = ts.with_timezone(&FixedOffset::east_opt(0).unwrap());
        let stored = event.new();
        assert_eq!(stored.ts, ts);
        assert_eq!(stored.ts.offset().local_minus_utc(), 8 * 3600);
    }

    #[test]
    fn new_replaces_a_future_client_ts() {
        let mut event = event("note", LogLevel::Info);
        event.ts = hkt_now() + Duration::hours(1);
        let before = hkt_now();
        let stored = event.new();
        assert!(stored.ts >= before && stored.ts <= hkt_now());
    }

    #[test]
    fn new_keeps_the_client_action() {
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
            let stored = event(action, LogLevel::Info).new();
            assert_eq!(stored.action, action);
        }
    }

    #[test]
    fn new_filters_info() {
        let mut client = event("/files", LogLevel::Info);
        client.info.insert("bad key".to_string(), json!("no"));
        client.info.insert("k01".to_string(), json!("kept"));
        client.info.insert("k02".to_string(), json!("a\nb"));
        client.info.insert("k03".to_string(), json!("a\rb"));
        client
            .info
            .insert("k07".to_string(), json!("x".repeat(250)));
        client.info.insert("k08".to_string(), json!("ninth"));
        client.info.insert("k09".to_string(), json!(true));
        let stored = client.new();
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
        scalars.info.insert("a/b".to_string(), json!("no"));
        scalars.info.insert("a_b-c.d".to_string(), json!("yes"));
        scalars.info.insert("a".repeat(65), json!("long-key"));
        scalars.info.insert("count".to_string(), json!(12));
        scalars.info.insert("flag".to_string(), json!(true));
        scalars.info.insert("off".to_string(), json!(false));
        scalars.info.insert("ratio".to_string(), json!(1.5));
        let stored = scalars.new();
        assert_eq!(stored.info.get("a/b"), None);
        assert_eq!(stored.info[&"a".repeat(65)], json!("long-key"));
        assert_eq!(stored.info["a_b-c.d"], json!("yes"));
        assert_eq!(stored.info["count"], json!("12"));
        assert_eq!(stored.info["flag"], json!("true"));
        assert_eq!(stored.info["off"], json!("false"));
        assert_eq!(stored.info["ratio"], json!("1.5"));

        let mut text = event("/files", LogLevel::Info);
        text.info.insert(String::new(), json!("no"));
        text.info.insert("café".to_string(), json!("no"));
        text.info.insert("ok!".to_string(), json!("no"));
        let long = "中".repeat(67);
        text.info.insert("text".to_string(), json!(long.clone()));
        let stored = text.new();
        assert_eq!(stored.info.get(""), None);
        assert_eq!(stored.info.get("café"), None);
        assert_eq!(stored.info.get("ok!"), None);
        assert_eq!(stored.info["text"], json!(long));

        let mut later = event("/files", LogLevel::Info);
        for index in 0..8 {
            later.info.insert(format!("bad {index}"), json!("no"));
        }
        later.info.insert("zzzz".to_string(), json!("yes"));
        let stored = later.new();
        assert_eq!(stored.info.len(), 1);
        assert_eq!(stored.info["zzzz"], json!("yes"));
    }
}
