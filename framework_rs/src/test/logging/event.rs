use chrono::{DateTime, Duration, FixedOffset};
use serde_json::{Map, Value};

use crate::time::hkt_now;

use crate::logging::event::*;
use serde_json::json;

#[test]
fn client_action_and_info_are_bounded() {
    let batch_event = |action: &str, info: Value| -> LogEvent {
        let mut client = LogEvent::info(action);
        client.extra = info.as_object().cloned().unwrap_or_default();
        client.from_client()
    };
    let forged = batch_event("a\nINFO forged\r", json!({}));
    assert!(!forged.action().contains(['\n', '\r']));
    assert_eq!(batch_event(&"x".repeat(500), json!({})).action().len(), 128);
    let long = "y".repeat(2049);
    assert!(batch_event("a", json!({ "k": long })).extra().is_empty());
    let many: Map<String, Value> = (0..100).map(|n| (format!("k{n}"), json!("v"))).collect();
    assert_eq!(batch_event("a", Value::Object(many)).extra().len(), 32);
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
    let mut event = LogEvent::info("GET /health");
    event.ts = DateTime::parse_from_rfc3339("2026-10-18T20:00:00+08:00").unwrap();
    event.id = "req-1".to_string();
    event
        .context
        .insert("ip".to_string(), "203.0.113.8".to_string().into());
    event
        .context
        .insert("user_agent".to_string(), "demo-agent".to_string().into());
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
    let value: Value = serde_json::from_str(&json).unwrap();
    assert!(value.get("app").is_none());
    assert!(value.get("project").is_none());
    assert_eq!(value["id"], "");
    assert!(value["context"].as_object().unwrap().is_empty());
    assert_eq!(value["action"], "");
    assert!(value.get("request").is_none());
    assert!(value["latency_ms"].is_null());
    assert_eq!(value["level"], "warn");
    assert!(value["extra"].as_object().unwrap().is_empty());
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
    assert_eq!(event.extra()["region"], "asia");
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
fn a_sanitized_event_ts_has_the_hkt_offset() {
    let stored = LogEvent::info("/files").from_client();
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
fn context_and_extra_accept_only_a_string_a_number_or_a_bool() {
    for field in ["context", "extra"] {
        for good in [json!("text"), json!(1), json!(1.5), json!(true)] {
            let mut value = serde_json::to_value(sample()).unwrap();
            value[field]["k"] = good.clone();
            let event = serde_json::from_value::<LogEvent>(value).unwrap();
            let fields = if field == "context" {
                event.context()
            } else {
                event.extra()
            };
            assert_eq!(fields["k"], good);
        }
        for bad in [json!(null), json!({"a": 1}), json!([1])] {
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
fn new_drops_untrusted_fields() {
    let mut event = LogEvent::warn("GET /download?sig=abc#top");
    event.ts = DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z").unwrap();
    event.id = "client-id".to_string();
    event
        .context
        .insert("ip".to_string(), "203.0.113.8".to_string().into());
    event.latency_ms = Some(42);
    let before = hkt_now();
    let stored = event.from_client();
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
    let mut event = LogEvent::info("note");
    let ts = hkt_now() - Duration::minutes(5);
    event.ts = ts.with_timezone(&FixedOffset::east_opt(0).unwrap());
    let stored = event.from_client();
    assert_eq!(stored.ts, ts);
    assert_eq!(stored.ts.offset().local_minus_utc(), 8 * 3600);
}

#[test]
fn new_replaces_a_future_client_ts() {
    let mut event = LogEvent::info("note");
    event.ts = hkt_now() + Duration::hours(1);
    let before = hkt_now();
    let stored = event.from_client();
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
        let stored = LogEvent::info(action).from_client();
        assert_eq!(stored.action, action);
    }
}

#[test]
fn new_filters_info() {
    let mut client = LogEvent::info("/files");
    client.extra.insert("bad key".to_string(), json!("no"));
    client.extra.insert("k01".to_string(), json!("kept"));
    client.extra.insert("k02".to_string(), json!("a\nb"));
    client.extra.insert("k03".to_string(), json!("a\rb"));
    client
        .extra
        .insert("k07".to_string(), json!("x".repeat(250)));
    client.extra.insert("k08".to_string(), json!("ninth"));
    client.extra.insert("k09".to_string(), json!(true));
    let stored = client.from_client();
    assert_eq!(
        stored.extra.keys().cloned().collect::<Vec<_>>(),
        vec![
            "k01".to_string(),
            "k07".to_string(),
            "k08".to_string(),
            "k09".to_string(),
        ]
    );
    assert_eq!(stored.extra["k01"], json!("kept"));
    assert_eq!(stored.extra["k07"], json!("x".repeat(250)));
    assert_eq!(stored.extra["k08"], json!("ninth"));
    assert_eq!(stored.extra["k09"], json!("true"));
    assert!(!stored.extra.contains_key("bad key"));

    let mut scalars = LogEvent::error("/files", "boom");
    scalars.extra.insert("a/b".to_string(), json!("no"));
    scalars.extra.insert("a_b-c.d".to_string(), json!("yes"));
    scalars.extra.insert("a".repeat(65), json!("long-key"));
    scalars.extra.insert("count".to_string(), json!(12));
    scalars.extra.insert("flag".to_string(), json!(true));
    scalars.extra.insert("off".to_string(), json!(false));
    scalars.extra.insert("ratio".to_string(), json!(1.5));
    let stored = scalars.from_client();
    assert_eq!(stored.extra.get("a/b"), None);
    assert_eq!(stored.extra[&"a".repeat(65)], json!("long-key"));
    assert_eq!(stored.extra["a_b-c.d"], json!("yes"));
    assert_eq!(stored.extra["count"], json!("12"));
    assert_eq!(stored.extra["flag"], json!("true"));
    assert_eq!(stored.extra["off"], json!("false"));
    assert_eq!(stored.extra["ratio"], json!("1.5"));

    let mut text = LogEvent::info("/files");
    text.extra.insert(String::new(), json!("no"));
    text.extra.insert("café".to_string(), json!("no"));
    text.extra.insert("ok!".to_string(), json!("no"));
    let long = "中".repeat(67);
    text.extra.insert("text".to_string(), json!(long.clone()));
    let stored = text.from_client();
    assert_eq!(stored.extra.get(""), None);
    assert_eq!(stored.extra.get("café"), None);
    assert_eq!(stored.extra.get("ok!"), None);
    assert_eq!(stored.extra["text"], json!(long));

    let mut later = LogEvent::info("/files");
    for index in 0..8 {
        later.extra.insert(format!("bad {index}"), json!("no"));
    }
    later.extra.insert("zzzz".to_string(), json!("yes"));
    let stored = later.from_client();
    assert_eq!(stored.extra.len(), 1);
    assert_eq!(stored.extra["zzzz"], json!("yes"));
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
    assert_eq!(error.id(), "");
    assert!(error.context().is_empty());
    assert!(error.extra().is_empty());
}

#[test]
fn a_constructor_does_not_truncate_the_action() {
    let long = "x".repeat(500);
    assert_eq!(LogEvent::info(long.clone()).action(), long);
    let message = LogEvent::error("a", long.clone());
    assert_eq!(message.error_message(), Some(long.as_str()));
}

#[test]
fn a_client_error_event_without_a_message_gets_a_placeholder() {
    let mut value = serde_json::to_value(LogEvent::error("x", "boom")).unwrap();
    value.as_object_mut().unwrap().remove("error_message");
    let parsed: LogEvent = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(parsed.error_message(), None);
    assert_eq!(
        parsed.from_client().error_message(),
        Some("no error message")
    );
    for blank in [json!(null), json!(""), json!("  "), json!("\u{1}\n")] {
        value["error_message"] = blank;
        let parsed: LogEvent = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(
            parsed.from_client().error_message(),
            Some("no error message")
        );
    }
    value["error_message"] = json!("boom");
    let parsed: LogEvent = serde_json::from_value(value).unwrap();
    assert_eq!(parsed.from_client().error_message(), Some("boom"));
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
    value["error_message"] = json!("slow");
    let parsed: LogEvent = serde_json::from_value(value).unwrap();
    assert_eq!(parsed.error_message(), Some("slow"));
}

#[test]
fn new_keeps_a_bounded_error_message() {
    let mut client = LogEvent::error("x", "boom");
    client.error_message = Some(format!("a\nb{}", "c".repeat(1500)));
    let stored = client.from_client();
    let message = stored.error_message().unwrap();
    assert!(!message.contains('\n'));
    assert_eq!(message.chars().count(), 1024);
    let mut warn = LogEvent::warn("x");
    warn.error_message = Some("y".repeat(2000));
    assert_eq!(warn.from_client().error_message().unwrap().len(), 1024);
}

#[test]
fn a_non_string_error_message_reads_as_none() {
    for bad in [
        json!(123),
        json!({"a": 1}),
        json!([1]),
        json!(true),
        json!(null),
    ] {
        let mut value = serde_json::to_value(LogEvent::error("x", "boom")).unwrap();
        value["error_message"] = bad;
        let parsed: LogEvent = serde_json::from_value(value).unwrap();
        assert_eq!(parsed.error_message(), None);
        assert_eq!(
            parsed.from_client().error_message(),
            Some("no error message")
        );
    }
}

#[test]
fn from_client_turns_a_blank_warn_message_into_none() {
    for blank in ["", "  ", "\u{1}\n\t"] {
        let mut warn = LogEvent::warn("x");
        warn.error_message = Some(blank.to_string());
        assert_eq!(warn.from_client().error_message(), None, "{blank:?}");
    }
}

#[test]
fn from_client_drops_an_error_message_on_an_info_event() {
    let mut client = LogEvent::info("x");
    client.error_message = Some("boom".to_string());
    assert_eq!(client.from_client().error_message(), None);
    let mut warn = LogEvent::warn("x");
    warn.error_message = Some("slow".to_string());
    assert_eq!(warn.from_client().error_message(), Some("slow"));
}
