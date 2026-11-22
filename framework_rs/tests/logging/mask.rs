use ian_framework_rs::logging::{LogEvent, Logger};

use crate::logging::{buffered_events, default_shipper, http_shipper, logger};

fn buffered() -> Logger {
    logger(http_shipper("api", "https://logs.example"))
}

#[test]
fn invalid_mask_pattern_returns_an_exception() {
    let err = Logger::new(default_shipper("api"), vec!["(".to_string()])
        .err()
        .unwrap();
    assert_eq!(err.code(), "MASK_PATTERN_INVALID");
    assert!(err.message().contains("is not a valid regex"));
}

#[test]
fn record_masks_the_value_of_a_matching_key() {
    let shipper = Logger::new(
        default_shipper("api"),
        vec![r"(?i)^(password|token)$".to_string()],
    )
    .unwrap();
    let mut event = LogEvent::info("login");
    event.extra.insert("password".to_string(), "hunter2".into());
    event.extra.insert("TOKEN".to_string(), 1234u64.into());
    event.extra.insert("user".to_string(), "alice".into());
    shipper.record(event);
    let stored = buffered_events(&shipper).remove(0);
    assert_eq!(stored.extra["password"], serde_json::json!("***"));
    assert_eq!(stored.extra["TOKEN"], serde_json::json!("***"));
    assert_eq!(stored.extra["user"], serde_json::json!("alice"));
}

#[test]
fn record_masks_a_pair_of_a_project_pattern_in_string_values() {
    let shipper = Logger::new(
        default_shipper("api"),
        vec![r"(?i)sid".to_string(), r"(?i)otp".to_string()],
    )
    .unwrap();
    let event = LogEvent::info("POST /login?otp=1")
        .with_context("request_url", "/login?SID=abc&otp=1&page=2")
        .with_extra("note", "user password=a, otp: 123456");
    shipper.record(event);
    let stored = buffered_events(&shipper).remove(0);
    assert_eq!(stored.action, "POST /login?otp=1");
    assert_eq!(stored.context["request_url"], "/login?***&***&page=2");
    assert_eq!(stored.extra["note"], "user password=a, ***");
}

#[test]
fn record_keeps_a_pattern_word_that_is_not_in_a_pair() {
    let shipper = buffered();
    let event = LogEvent::info("GET /")
        .with_context("request_url", "/api/tokens/list")
        .with_extra("note", "the password rule and the Cookie banner");
    shipper.record(event);
    let stored = buffered_events(&shipper).remove(0);
    assert_eq!(stored.context["request_url"], "/api/tokens/list");
    assert_eq!(
        stored.extra["note"],
        "the password rule and the Cookie banner"
    );
}

#[test]
fn default_patterns_mask_the_value_of_a_name_value_pair() {
    let shipper = buffered();
    let mut event = LogEvent::info("GET /");
    event.context.insert(
        "request_url".to_string(),
        "/x?a=1&access_token=abc123&page=2&Password=hunter2"
            .to_string()
            .into(),
    );
    shipper.record(event);
    let stored = buffered_events(&shipper).remove(0);
    assert_eq!(stored.context["request_url"], "/x?a=1&***&page=2&***");
}

#[test]
fn record_without_mask_patterns_keeps_the_text() {
    let shipper = Logger::new(default_shipper("api"), Vec::new()).unwrap();
    let mut event = LogEvent::info("GET /");
    event
        .extra
        .insert("note".to_string(), "password=hunter2".into());
    shipper.record(event);
    let stored = buffered_events(&shipper).remove(0);
    assert_eq!(stored.extra["note"], serde_json::json!("password=hunter2"));
}

#[test]
fn default_patterns_mask_the_value_of_a_json_member() {
    let shipper = buffered();
    let mut event = LogEvent::info("POST /login");
    event.extra.insert(
        "body".to_string(),
        r#"{"user":"a","password":"hun\"ter2","n":{"apiKey": 12345,"page":2}}"#.into(),
    );
    shipper.record(event);
    let stored = buffered_events(&shipper).remove(0);
    assert_eq!(
        stored.extra["body"],
        serde_json::json!(r#"{"user":"a",***,"n":{***,"page":2}}"#)
    );
}

#[test]
fn default_patterns_mask_the_value_after_a_name_and_a_colon() {
    let shipper = buffered();
    let mut event = LogEvent::info("GET /");
    event.extra.insert(
        "headers".to_string(),
        "Accept: text/html\nAuthorization: Bearer abc.def\nCookie: a=1; sid=xyz".into(),
    );
    event.extra.insert(
        "note".to_string(),
        r#"{"msg":"X-Api-Key: abc","id":5}"#.into(),
    );
    shipper.record(event);
    let stored = buffered_events(&shipper).remove(0);
    assert_eq!(
        stored.extra["headers"],
        serde_json::json!("Accept: text/html\n***\n***")
    );
    assert_eq!(
        stored.extra["note"],
        serde_json::json!(r#"{"msg":"***","id":5}"#)
    );
}

#[test]
fn record_masks_a_pair_in_the_error_message_and_keeps_the_other_text() {
    let shipper = buffered();
    let event = LogEvent::error(
        "GET /",
        "password hash failed for /x?token=abc123\n0: ERROR [PASSWORD_HASH_FAILED] failed at src/auth/token.rs:12",
    );
    shipper.record(event);
    let stored = buffered_events(&shipper).remove(0);
    assert_eq!(
        stored.error_message.as_deref(),
        Some(
            "password hash failed for /x?***\n0: ERROR [PASSWORD_HASH_FAILED] failed at src/auth/token.rs:12"
        )
    );
}
