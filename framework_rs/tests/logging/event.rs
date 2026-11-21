use chrono::DateTime;
use serde_json;

use ian_framework_rs::logging::*;

fn sample() -> LogEvent {
    let mut event = LogEvent::info("GET /health");
    event.ts = DateTime::parse_from_rfc3339("2026-10-18T20:00:00+08:00").unwrap();
    event.id = "req-1".to_string();
    event
        .context
        .insert("ip".to_string(), "203.0.113.8".to_string().into());
    event
        .context
        .insert("user_agent".to_string(), "demo-agent".to_string().into());
    event.latency_ms = 12;
    event.extra.insert("region".to_string(), "asia".into());
    event
}

#[test]
fn json_object_has_every_schema_field() {
    let json = serde_json::to_string(&sample()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    for key in [
        "ts",
        "id",
        "action",
        "latency_ms",
        "level",
        "context",
        "extra",
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
    assert_eq!(value["extra"]["region"], "asia");
}

#[test]
fn empty_http_fields_are_null_or_empty() {
    let json = serde_json::to_string(&LogEvent::warn("")).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(value.get("app").is_none());
    assert!(value.get("project").is_none());
    crate::logging::assert_uuid_v7(value["id"].as_str().unwrap());
    assert!(value["context"].as_object().unwrap().is_empty());
    assert_eq!(value["action"], "");
    assert!(value.get("request").is_none());
    assert_eq!(value["latency_ms"], 0);
    assert_eq!(value["level"], "warn");
    assert!(value["extra"].as_object().unwrap().is_empty());
}

#[test]
fn json_object_round_trips() {
    let event = sample();
    let parsed: LogEvent = serde_json::from_str(&serde_json::to_string(&event).unwrap()).unwrap();
    assert_eq!(parsed, event);
}

#[test]
fn a_new_event_ts_has_the_hkt_offset() {
    let value = serde_json::to_value(LogEvent::info("")).unwrap();
    assert!(value["ts"].as_str().unwrap().ends_with("+08:00"));
}

#[test]
fn a_utc_ts_is_accepted() {
    let mut value = serde_json::to_value(sample()).unwrap();
    value["ts"] = serde_json::json!("2026-10-18T12:00:00Z");
    let parsed: LogEvent = serde_json::from_value(value).unwrap();
    assert_eq!(parsed.ts, sample().ts);
    assert_eq!(parsed.ts.offset().local_minus_utc(), 0);
}

#[test]
fn context_and_extra_accept_only_a_string_a_number_or_a_bool() {
    for field in ["context", "extra"] {
        for good in [
            serde_json::json!("text"),
            serde_json::json!(1),
            serde_json::json!(1.5),
            serde_json::json!(true),
        ] {
            let mut value = serde_json::to_value(sample()).unwrap();
            value[field]["k"] = good.clone();
            let event = serde_json::from_value::<LogEvent>(value).unwrap();
            let fields = if field == "context" {
                event.context
            } else {
                event.extra
            };
            assert_eq!(fields["k"], good);
        }
        for bad in [
            serde_json::json!(null),
            serde_json::json!({"a": 1}),
            serde_json::json!([1]),
        ] {
            let mut value = serde_json::to_value(sample()).unwrap();
            value[field]["k"] = bad;
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
fn constructors_set_the_level_and_the_message() {
    let info = LogEvent::info("a");
    assert_eq!((info.level(), info.error_message()), (LogLevel::Info, None));
    let warn = LogEvent::warn("a");
    assert_eq!((warn.level(), warn.error_message()), (LogLevel::Warn, None));
    let warn = LogEvent::warn_with("a", "slow");
    assert_eq!(warn.error_message(), Some("slow"));
    let error = LogEvent::error("a", "boom");
    assert_eq!(error.level(), LogLevel::Error);
    assert_eq!(error.error_message(), Some("boom"));
    crate::logging::assert_uuid_v7(error.id());
    assert!(error.context.is_empty());
    assert!(error.extra.is_empty());
}

#[test]
fn a_constructor_does_not_truncate_the_action() {
    let long = "x".repeat(500);
    assert_eq!(LogEvent::info(long.clone()).action(), long);
    let message = LogEvent::error("a", long.clone());
    assert_eq!(message.error_message(), Some(long.as_str()));
}

#[test]
fn a_warn_event_may_have_error_message() {
    let mut value = serde_json::to_value(LogEvent::warn("x")).unwrap();
    assert!(value["error_message"].is_null());
    assert_eq!(
        serde_json::from_value::<LogEvent>(value.clone())
            .unwrap()
            .error_message(),
        None
    );
    value["error_message"] = serde_json::json!("slow");
    let parsed: LogEvent = serde_json::from_value(value).unwrap();
    assert_eq!(parsed.error_message(), Some("slow"));
}

#[test]
fn a_non_string_error_message_is_rejected() {
    for bad in [
        serde_json::json!(123),
        serde_json::json!({"a": 1}),
        serde_json::json!([1]),
        serde_json::json!(true),
    ] {
        let mut value = serde_json::to_value(LogEvent::error("x", "boom")).unwrap();
        value["error_message"] = bad;
        assert!(serde_json::from_value::<LogEvent>(value).is_err());
    }
    let mut value = serde_json::to_value(LogEvent::error("x", "boom")).unwrap();
    value["error_message"] = serde_json::json!(null);
    let parsed: LogEvent = serde_json::from_value(value).unwrap();
    assert_eq!(parsed.error_message(), None);
}
