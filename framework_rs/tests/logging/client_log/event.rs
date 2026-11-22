use axum::http::HeaderMap;
use chrono::{DateTime, Duration, FixedOffset};
use serde_json;

use ian_framework_rs::logging::client_log::event::snake_case;
use ian_framework_rs::logging::log_shipper::SHIP_CHUNK;
use ian_framework_rs::logging::{ClientLog, LogEvent, LogLevel};
use ian_framework_rs::time::hkt_now;

use crate::logging::{assert_uuid_v7, client_log_event};

impl ClientChecked for LogEvent {
    fn client_checked(&self) -> LogEvent {
        let log = ClientLog {
            events: vec![client_log_event(self)],
        };
        log.log_events(&HeaderMap::new()).unwrap().remove(0)
    }
}

trait ClientChecked {
    fn client_checked(&self) -> LogEvent;
}

#[test]
fn log_events_rejects_an_empty_list() {
    let err = ClientLog { events: vec![] }
        .log_events(&HeaderMap::new())
        .unwrap_err();
    assert_eq!(err.code(), "BAD_REQUEST");
}

#[test]
fn log_events_returns_sanitized_events() {
    let log = ClientLog {
        events: vec![
            client_log_event(&LogEvent::info("/a?x=1")),
            client_log_event(&LogEvent::error("#frag", "boom")),
        ],
    };
    let stored = log.log_events(&HeaderMap::new()).unwrap();
    assert_eq!(stored.len(), 2);
    assert_eq!(stored[0].action, "/a?x=1");
    assert_eq!(stored[0].level, LogLevel::Info);
    assert_uuid_v7(&stored[0].id);
    assert_eq!(stored[0].context.len(), 2);
    assert_eq!(stored[0].context["ip"], "-");
    assert_eq!(stored[0].latency_ms, 0);
    assert_eq!(stored[1].action, "#frag");
    assert_eq!(stored[1].level, LogLevel::Error);
}

#[test]
fn log_events_accepts_a_list_at_the_ship_chunk() {
    let events: Vec<_> = (0..SHIP_CHUNK)
        .map(|_| client_log_event(&LogEvent::info("/ok")))
        .collect();
    let stored = ClientLog { events }.log_events(&HeaderMap::new()).unwrap();
    assert_eq!(stored.len(), SHIP_CHUNK);
    assert_uuid_v7(&stored[0].id);
}

#[test]
fn log_events_rejects_a_list_above_the_ship_chunk() {
    let events: Vec<_> = (0..=SHIP_CHUNK)
        .map(|_| client_log_event(&LogEvent::info("/ok")))
        .collect();
    assert_eq!(events.len(), SHIP_CHUNK + 1);
    let err = ClientLog { events }
        .log_events(&HeaderMap::new())
        .unwrap_err();
    assert_eq!(err.code(), "BAD_REQUEST");
}

#[test]
fn client_action_and_info_are_bounded() {
    let batch_event = |action: &str, info: serde_json::Value| -> LogEvent {
        let mut wire = serde_json::to_value(LogEvent::info(action)).unwrap();
        wire["extra"] = info;
        serde_json::from_value::<LogEvent>(wire)
            .unwrap()
            .client_checked()
    };
    let forged = batch_event("a\nINFO forged\r", serde_json::json!({}));
    assert!(!forged.action().contains(['\n', '\r']));
    assert_eq!(
        batch_event(&"x".repeat(500), serde_json::json!({}))
            .action()
            .len(),
        128
    );
    let long = "y".repeat(2049);
    assert!(
        batch_event("a", serde_json::json!({ "k": long }))
            .extra
            .is_empty()
    );
    let many: serde_json::Map<String, serde_json::Value> = (0..100)
        .map(|n| (format!("k{n}"), serde_json::json!("v")))
        .collect();
    assert_eq!(
        batch_event("a", serde_json::Value::Object(many))
            .extra
            .len(),
        32
    );
}

#[test]
fn a_sanitized_event_ts_has_the_hkt_offset() {
    let stored = LogEvent::info("/files").client_checked();
    let value = serde_json::to_value(stored).unwrap();
    assert!(value["ts"].as_str().unwrap().ends_with("+08:00"));
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
    assert_uuid_v7(&stored.id);
    assert_ne!(stored.id, event.id);
    assert_eq!(stored.context.len(), 2);
    assert_eq!(stored.context["ip"], "-");
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
    assert_eq!(stored.extra["k01"], serde_json::json!("kept"));
    assert_eq!(stored.extra["k07"], serde_json::json!("x".repeat(250)));
    assert_eq!(stored.extra["k08"], serde_json::json!("ninth"));
    assert_eq!(stored.extra["k09"], serde_json::json!(true));
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
    assert_eq!(stored.extra[&"a".repeat(65)], serde_json::json!("long-key"));
    assert_eq!(stored.extra["a_b-c.d"], serde_json::json!("yes"));
    assert_eq!(stored.extra["count"], serde_json::json!(12));
    assert_eq!(stored.extra["flag"], serde_json::json!(true));
    assert_eq!(stored.extra["off"], serde_json::json!(false));
    assert_eq!(stored.extra["ratio"], serde_json::json!(1.5));

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
    assert_eq!(stored.extra["text"], serde_json::json!(long));

    let mut later = LogEvent::info("/files");
    for index in 0..8 {
        later.extra.insert(format!("bad {index}"), "no".into());
    }
    later.extra.insert("zzzz".to_string(), "yes".into());
    let stored = later.client_checked();
    assert_eq!(stored.extra.len(), 1);
    assert_eq!(stored.extra["zzzz"], serde_json::json!("yes"));
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
    for blank in [
        serde_json::json!(null),
        serde_json::json!(""),
        serde_json::json!("  "),
        serde_json::json!("\u{1}\n"),
    ] {
        value["error_message"] = blank;
        let parsed: LogEvent = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(
            parsed.client_checked().error_message(),
            Some("unknown error")
        );
    }
    value["error_message"] = serde_json::json!("boom");
    let parsed: LogEvent = serde_json::from_value(value).unwrap();
    assert_eq!(parsed.client_checked().error_message(), Some("boom"));
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
fn snake_case_converts_camel_case_keys() {
    for (input, expected) in [
        ("userId", "user_id"),
        ("HTTPServer", "http_server"),
        ("pageX2", "page_x2"),
        ("already_snake", "already_snake"),
        ("a-b.cD", "a-b.c_d"),
        ("URL", "url"),
        ("", ""),
    ] {
        assert_eq!(snake_case(input), expected, "{input}");
    }
}

#[test]
fn client_context_and_extra_keys_become_snake_case() {
    let mut event = LogEvent::info("/files");
    event
        .context
        .insert("tenantId".to_string(), "my-app".into());
    event.extra.insert("retryCount".to_string(), 2u64.into());
    let stored = event.client_checked();
    assert_eq!(stored.context["tenant_id"], "my-app");
    assert_eq!(stored.extra["retry_count"], serde_json::json!(2));
    assert!(!stored.extra.contains_key("retryCount"));
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
    assert_eq!(stored.context["count"], serde_json::json!(3));
    assert_eq!(stored.context["ip"], "-");
    assert_eq!(stored.context["user_agent"], "-");
    assert!(!stored.context.contains_key("bad key"));
    assert!(!stored.context.contains_key("line"));
    assert_eq!(stored.context.len(), 4);
}

fn with_error_fields(event: &LogEvent, name: Option<&str>, stack: Option<&str>) -> Option<String> {
    let mut client = client_log_event(event);
    client.error_name = name.map(str::to_string);
    client.stack = stack.map(str::to_string);
    let log = ClientLog {
        events: vec![client],
    };
    let mut events = log.log_events(&HeaderMap::new()).unwrap();
    events.remove(0).error_message
}

#[test]
fn error_name_and_stack_go_in_the_error_message() {
    let error = LogEvent::error("x", "boom");
    assert_eq!(
        with_error_fields(
            &error,
            Some("TypeError"),
            Some("at a (app.js:1:2)\r\n\tat b\n")
        )
        .as_deref(),
        Some("TypeError: boom\nat a (app.js:1:2) \n at b")
    );
    assert_eq!(
        with_error_fields(&error, Some(" \n"), Some("  ")).as_deref(),
        Some("boom")
    );
    let warn = LogEvent::warn_with("x", "slow");
    assert_eq!(
        with_error_fields(&warn, Some("ApiError"), None).as_deref(),
        Some("ApiError: slow")
    );
}

#[test]
fn error_name_and_stack_are_bounded_and_need_a_message() {
    let error = LogEvent::error("x", "boom");
    let message =
        with_error_fields(&error, Some(&"N".repeat(100)), Some(&"s".repeat(5000))).unwrap();
    assert_eq!(message.len(), 64 + ": boom\n".len() + 4096);
    assert_eq!(
        with_error_fields(&LogEvent::info("x"), Some("TypeError"), Some("at a")),
        None
    );
    assert_eq!(
        with_error_fields(&LogEvent::warn("x"), Some("TypeError"), Some("at a")),
        None
    );
    let mut blank = LogEvent::error("x", "boom");
    blank.error_message = None;
    assert_eq!(
        with_error_fields(&blank, Some("TypeError"), None).as_deref(),
        Some("TypeError: unknown error")
    );
}

#[test]
fn an_event_without_the_error_name_and_the_stack_is_accepted() {
    let body = serde_json::json!({
        "ts": "2026-10-25T12:00:00Z",
        "action": "x",
        "latencyMs": null,
        "level": "error",
        "errorMessage": "boom",
        "context": {},
        "extra": {},
    });
    let event: ian_framework_rs::logging::ClientLogEvent = serde_json::from_value(body).unwrap();
    assert_eq!(event.error_name, None);
    assert_eq!(event.stack, None);
}
