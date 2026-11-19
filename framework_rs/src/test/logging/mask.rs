use crate::logging::{LogEvent, Logger};
use crate::test::logging::helpers::{buffered_events, default_shipper, http_shipper, logger};

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
fn record_masks_every_pattern_match_in_string_values() {
    let shipper = Logger::new(
        default_shipper("api"),
        vec![
            r"password=\S+".to_string(),
            r"(?i)bearer [A-Za-z0-9._-]+".to_string(),
        ],
    )
    .unwrap();
    let mut event = LogEvent::info("POST /login?password=x");
    event.context.insert(
        "user_agent".to_string(),
        "agent BEARER abc.def".to_string().into(),
    );
    event.extra.insert(
        "note".to_string(),
        "user password=a, BEARER b.c done".into(),
    );
    shipper.record(event);
    let stored = buffered_events(&shipper).remove(0);
    assert_eq!(stored.action, "POST /login?password=x");
    assert_eq!(stored.context["user_agent"], "agent ***");
    assert_eq!(stored.extra["note"], serde_json::json!("user *** *** done"));
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
