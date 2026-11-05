use axum::http::HeaderMap;
use chrono::{DateTime, Duration, FixedOffset};
use serde_json::{Map, Value};

use crate::time::hkt_now;

use crate::logging::event::*;
use crate::logging::ClientLog;
use serde_json::json;

trait ClientChecked {
    fn client_checked(&self) -> LogEvent;
}

impl ClientChecked for LogEvent {
    fn client_checked(&self) -> LogEvent {
        let log = ClientLog {
            events: vec![crate::test::logging::client_log_event(self)],
        };
        log.log_events(&HeaderMap::new()).unwrap().remove(0)
    }
}

#[test]
fn client_action_and_info_are_bounded() {
    let batch_event = |action: &str, info: Value| -> LogEvent {
        let mut wire = serde_json::to_value(LogEvent::info(action)).unwrap();
        wire["extra"] = info;
        serde_json::from_value::<LogEvent>(wire)
            .unwrap()
            .client_checked()
    };
    let forged = batch_event("a\nINFO forged\r", json!({}));
    assert!(!forged.action().contains(['\n', '\r']));
    assert_eq!(batch_event(&"x".repeat(500), json!({})).action().len(), 128);
    let long = "y".repeat(2049);
    assert!(batch_event("a", json!({ "k": long })).extra.is_empty());
    let many: Map<String, Value> = (0..100).map(|n| (format!("k{n}"), json!("v"))).collect();
    assert_eq!(batch_event("a", Value::Object(many)).extra.len(), 32);
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
    event.latency_ms = 12;
    event.extra.insert("region".to_string(), "asia".into());
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
    crate::test::logging::assert_uuid_v7(value["id"].as_str().unwrap());
    assert!(value["context"].as_object().unwrap().is_empty());
    assert_eq!(value["action"], "");
    assert!(value.get("request").is_none());
    assert_eq!(value["latency_ms"], 0);
    assert_eq!(value["level"], "warn");
    assert!(value["extra"].as_object().unwrap().is_empty());
}

#[test]
fn getters_expose_the_fields() {
    let event = sample();
    assert_eq!(event.ts(), sample().ts);
    assert_eq!(event.id(), "req-1");
    assert_eq!(event.action(), "GET /health");
    assert_eq!(event.latency_ms(), 12);
    assert_eq!(event.level(), LogLevel::Info);
    assert_eq!(event.context["ip"], "203.0.113.8");
    assert_eq!(event.extra["region"], "asia");
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
    let stored = LogEvent::info("/files").client_checked();
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
                event.context
            } else {
                event.extra
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
    event.latency_ms = 42;
    let before = hkt_now();
    let stored = event.client_checked();
    let after = hkt_now();
    assert_eq!(event.id, "client-id");
    crate::test::logging::assert_uuid_v7(&stored.id);
    assert_ne!(stored.id, event.id);
    assert_eq!(stored.context.len(), 2);
    assert_eq!(stored.context["ip"], "");
    assert_eq!(stored.latency_ms, 42);
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
    let stored = event.client_checked();
    assert_eq!(stored.ts, ts);
    assert_eq!(stored.ts.offset().local_minus_utc(), 8 * 3600);
}

#[test]
fn new_replaces_a_future_client_ts() {
    let mut event = LogEvent::info("note");
    event.ts = hkt_now() + Duration::hours(1);
    let before = hkt_now();
    let stored = event.client_checked();
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
        let stored = LogEvent::info(action).client_checked();
        assert_eq!(stored.action, action);
    }
}

#[test]
fn new_filters_info() {
    let mut client = LogEvent::info("/files");
    client.extra.insert("bad key".to_string(), "no".into());
    client.extra.insert("k01".to_string(), "kept".into());
    client.extra.insert("k02".to_string(), "a\nb".into());
    client.extra.insert("k03".to_string(), "a\rb".into());
    client
        .extra
        .insert("k07".to_string(), "x".repeat(250).into());
    client.extra.insert("k08".to_string(), "ninth".into());
    client.extra.insert("k09".to_string(), true.into());
    let stored = client.client_checked();
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
    assert_eq!(stored.extra["k09"], json!(true));
    assert!(!stored.extra.contains_key("bad key"));

    let mut scalars = LogEvent::error("/files", "boom");
    scalars.extra.insert("a/b".to_string(), "no".into());
    scalars.extra.insert("a_b-c.d".to_string(), "yes".into());
    scalars.extra.insert("a".repeat(65), "long-key".into());
    scalars.extra.insert("count".to_string(), 12u64.into());
    scalars.extra.insert("flag".to_string(), true.into());
    scalars.extra.insert("off".to_string(), false.into());
    scalars.extra.insert("ratio".to_string(), 1.5.into());
    let stored = scalars.client_checked();
    assert_eq!(stored.extra.get("a/b"), None);
    assert_eq!(stored.extra[&"a".repeat(65)], json!("long-key"));
    assert_eq!(stored.extra["a_b-c.d"], json!("yes"));
    assert_eq!(stored.extra["count"], json!(12));
    assert_eq!(stored.extra["flag"], json!(true));
    assert_eq!(stored.extra["off"], json!(false));
    assert_eq!(stored.extra["ratio"], json!(1.5));

    let mut text = LogEvent::info("/files");
    text.extra.insert(String::new(), "no".into());
    text.extra.insert("café".to_string(), "no".into());
    text.extra.insert("ok!".to_string(), "no".into());
    let long = "中".repeat(67);
    text.extra.insert("text".to_string(), long.clone().into());
    let stored = text.client_checked();
    assert_eq!(stored.extra.get(""), None);
    assert_eq!(stored.extra.get("café"), None);
    assert_eq!(stored.extra.get("ok!"), None);
    assert_eq!(stored.extra["text"], json!(long));

    let mut later = LogEvent::info("/files");
    for index in 0..8 {
        later.extra.insert(format!("bad {index}"), "no".into());
    }
    later.extra.insert("zzzz".to_string(), "yes".into());
    let stored = later.client_checked();
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
    crate::test::logging::assert_uuid_v7(error.id());
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
fn a_client_error_event_without_a_message_gets_a_placeholder() {
    let mut value = serde_json::to_value(LogEvent::error("x", "boom")).unwrap();
    value.as_object_mut().unwrap().remove("error_message");
    let parsed: LogEvent = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(parsed.error_message(), None);
    assert_eq!(
        parsed.client_checked().error_message(),
        Some("unknown error")
    );
    for blank in [json!(null), json!(""), json!("  "), json!("\u{1}\n")] {
        value["error_message"] = blank;
        let parsed: LogEvent = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(
            parsed.client_checked().error_message(),
            Some("unknown error")
        );
    }
    value["error_message"] = json!("boom");
    let parsed: LogEvent = serde_json::from_value(value).unwrap();
    assert_eq!(parsed.client_checked().error_message(), Some("boom"));
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
    let stored = client.client_checked();
    let message = stored.error_message().unwrap();
    assert!(!message.contains('\n'));
    assert_eq!(message.chars().count(), 1024);
    let mut warn = LogEvent::warn("x");
    warn.error_message = Some("y".repeat(2000));
    assert_eq!(warn.client_checked().error_message().unwrap().len(), 1024);
}

#[test]
fn a_non_string_error_message_is_rejected() {
    for bad in [json!(123), json!({"a": 1}), json!([1]), json!(true)] {
        let mut value = serde_json::to_value(LogEvent::error("x", "boom")).unwrap();
        value["error_message"] = bad;
        assert!(serde_json::from_value::<LogEvent>(value).is_err());
    }
    let mut value = serde_json::to_value(LogEvent::error("x", "boom")).unwrap();
    value["error_message"] = json!(null);
    let parsed: LogEvent = serde_json::from_value(value).unwrap();
    assert_eq!(parsed.error_message(), None);
    assert_eq!(
        parsed.client_checked().error_message(),
        Some("unknown error")
    );
}

#[test]
fn from_client_turns_a_blank_warn_message_into_none() {
    for blank in ["", "  ", "\u{1}\n\t"] {
        let mut warn = LogEvent::warn("x");
        warn.error_message = Some(blank.to_string());
        assert_eq!(warn.client_checked().error_message(), None, "{blank:?}");
    }
}

#[test]
fn from_client_drops_an_error_message_on_an_info_event() {
    let mut client = LogEvent::info("x");
    client.error_message = Some("boom".to_string());
    assert_eq!(client.client_checked().error_message(), None);
    let mut warn = LogEvent::warn("x");
    warn.error_message = Some("slow".to_string());
    assert_eq!(warn.client_checked().error_message(), Some("slow"));
}

#[test]
fn client_context_is_kept_and_the_request_ip_and_user_agent_win() {
    let mut event = LogEvent::info("/files");
    event
        .context
        .insert("ip".to_string(), "203.0.113.8".to_string().into());
    event
        .context
        .insert("user_agent".to_string(), "fake-agent".into());
    event.context.insert("tenant".to_string(), "my-app".into());
    event.context.insert("bad key".to_string(), "no".into());
    event.context.insert("line".to_string(), "a\nb".into());
    event.context.insert("count".to_string(), 3u64.into());
    let stored = event.client_checked();
    assert_eq!(stored.context["tenant"], "my-app");
    assert_eq!(stored.context["count"], json!(3));
    assert_eq!(stored.context["ip"], "");
    assert_eq!(stored.context["user_agent"], "");
    assert!(!stored.context.contains_key("bad key"));
    assert!(!stored.context.contains_key("line"));
    assert_eq!(stored.context.len(), 4);
}
