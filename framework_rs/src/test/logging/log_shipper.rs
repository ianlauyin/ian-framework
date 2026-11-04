use std::time::Duration;

use crate::shutdown::ShutdownReceiver;

use crate::logging::{LogEvent, LogLevel, ShipperConfig, SHIP_CHUNK};

use chrono::{DateTime, Utc};

use crate::logging::log_shipper::*;
use crate::test::logging::buffered_events;

fn config(app: &str, endpoint: impl Into<String>) -> ShipperConfig {
    let mut config = ShipperConfig::new(app);
    config.endpoint = Some(endpoint.into());
    config
}

#[test]
fn defaults() {
    let shipper = LogShipper::new(ShipperConfig::new("api")).unwrap();
    assert_eq!(shipper.inner.app, "api");
    assert_eq!(SHIP_CHUNK, 1_000);
    assert_eq!(SHIP_TIMEOUT, Duration::from_secs(10));
}

#[test]
fn blank_endpoint_is_trace_only() {
    for endpoint in ["", "   "] {
        let shipper = LogShipper::new(config("api", endpoint)).unwrap();
        assert_eq!(shipper.inner.endpoint, None);
    }
    let unset = ShipperConfig::new("api");
    assert_eq!(LogShipper::new(unset).unwrap().inner.endpoint, None);
}

#[test]
fn endpoint_is_normalized() {
    let shipper = LogShipper::new(config("my-app", "https://logs.example/ingest/")).unwrap();
    assert_eq!(
        shipper.inner.endpoint.as_deref(),
        Some("https://logs.example/ingest")
    );
}

#[test]
fn invalid_config_is_rejected() {
    let cases = [
        config("", "https://logs.example"),
        config("a/b", "https://logs.example"),
        {
            let mut config = config("api", "https://logs.example");
            config.capacity = 0;
            config
        },
        {
            let mut config = config("api", "https://logs.example");
            config.ship_every = Duration::ZERO;
            config
        },
        config("api", "ftp://logs.example"),
        config("api", "https://logs.example/a b"),
        {
            let mut config = config("api", "https://logs.example");
            config.mask_patterns = vec!["(".to_string()];
            config
        },
    ];
    for config in cases {
        assert!(LogShipper::new(config).is_none());
    }
}

#[test]
fn a_plain_http_endpoint_reaches_this_machine_or_a_private_address() {
    for endpoint in [
        "http://localhost:8080",
        "http://127.0.0.1:8080",
        "http://[::1]:8080",
        "http://10.0.0.5:8080",
        "http://172.16.0.1:8080",
        "http://192.168.1.1:8080",
        "https://logs.example",
    ] {
        assert!(
            LogShipper::new(config("api", endpoint)).is_some(),
            "{endpoint}"
        );
    }
    for endpoint in [
        "http://logs.example",
        "http://8.8.8.8:8080",
        "http://11.0.0.1:8080",
        "http://172.15.0.1:8080",
        "http://172.32.0.1:8080",
    ] {
        assert!(
            LogShipper::new(config("api", endpoint)).is_none(),
            "{endpoint}"
        );
    }
}

#[test]
fn endpoint_with_user_info_is_rejected() {
    for endpoint in [
        "https://user:pass@logs.example",
        "https://user@logs.example",
        "https://:pass@logs.example",
    ] {
        assert!(LogShipper::new(config("api", endpoint)).is_none());
    }
}

#[test]
fn invalid_post_api_key_is_rejected() {
    for post_api_key in ["log post key", "log\u{7f}key"] {
        let mut config = config("api", "https://logs.example");
        config.post_api_key = Some(post_api_key.to_string());
        assert!(LogShipper::new(config).is_none());
    }
}

#[test]
fn debug_hides_the_post_api_key() {
    let mut with_key = config("api", "https://logs.example");
    with_key.post_api_key = Some("log-post-key".to_string());
    let shipper = LogShipper::new(with_key).unwrap();
    let debug = format!("{shipper:?}");
    assert!(!debug.contains("log-post-key"));
    assert!(debug.contains("post_api_key: Some(\"***\")"));
    let unset = LogShipper::new(config("api", "https://logs.example")).unwrap();
    assert!(format!("{unset:?}").contains("post_api_key: None"));
}

fn buffered() -> LogShipper {
    LogShipper::new(config("api", "https://logs.example")).unwrap()
}

fn actions_of(events: &[LogEvent]) -> Vec<&str> {
    events.iter().map(|event| event.action.as_str()).collect()
}

fn capacity_warn_count(events: &[LogEvent]) -> usize {
    events
        .iter()
        .filter(|event| event.action == CAPACITY_WARN_NOTE)
        .count()
}

fn assert_capacity_warn(event: &LogEvent) {
    assert_eq!(event.level, LogLevel::Warn);
    assert_eq!(event.id, "");
    assert!(event.context.is_empty());
    assert_eq!(event.action, CAPACITY_WARN_NOTE);
    assert_eq!(event.latency_ms, None);
    assert!(event.extra.is_empty());
}

fn assert_no_scope_keys(line: &str) {
    let value: serde_json::Value = serde_json::from_str(line).unwrap();
    assert!(value.get("app").is_none());
    assert!(value.get("project").is_none());
}

#[test]
fn record_omits_scope_keys() {
    let shipper = buffered();
    shipper.record(LogEvent::info("kept"));
    let events = buffered_events(&shipper);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].action, "kept");
    assert_eq!(shipper.inner.app, "api");
    assert_no_scope_keys(&serde_json::to_string(&events[0]).unwrap());
}

#[test]
fn record_masks_every_pattern_match_in_context_and_info_values() {
    let mut config = config("api", "https://logs.example");
    config.mask_patterns = vec![
        r"password=\S+".to_string(),
        r"(?i)bearer [A-Za-z0-9._-]+".to_string(),
    ];
    let shipper = LogShipper::new(config).unwrap();
    let mut event = LogEvent::info("POST /login?password=x");
    event.id = "password=1".to_string();
    event.context.insert(
        "user_agent".to_string(),
        "agent BEARER abc.def".to_string().into(),
    );
    event
        .extra
        .insert("password=k".to_string(), "Bearer t0k".into());
    event.extra.insert(
        "note".to_string(),
        "user password=a, BEARER b.c done".into(),
    );
    shipper.record(event);
    let stored = buffered_events(&shipper).remove(0);
    assert_eq!(stored.action, "POST /login?password=x");
    assert_eq!(stored.id, "password=1");
    assert_eq!(stored.context["user_agent"], "agent ***");
    assert_eq!(stored.extra["password=k"], serde_json::json!("***"));
    assert_eq!(stored.extra["note"], serde_json::json!("user *** *** done"));
}

#[test]
fn default_patterns_mask_the_value_of_a_name_value_pair() {
    let shipper = LogShipper::new(config("api", "https://logs.example")).unwrap();
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
fn record_masks_the_value_of_a_matching_info_key() {
    let mut config = config("api", "https://logs.example");
    config.mask_patterns = vec![r"(?i)^(password|token)$".to_string()];
    let shipper = LogShipper::new(config).unwrap();
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
fn record_without_mask_patterns_keeps_the_text() {
    let mut config = ShipperConfig::new("api");
    config.endpoint = Some("https://logs.example".to_string());
    config.mask_patterns = Vec::new();
    let shipper = LogShipper::new(config).unwrap();
    let mut event = LogEvent::info("password=hunter2");
    event
        .extra
        .insert("note".to_string(), "password=hunter2".into());
    shipper.record(event);
    let stored = buffered_events(&shipper).remove(0);
    assert_eq!(stored.action, "password=hunter2");
    assert_eq!(stored.extra["note"], serde_json::json!("password=hunter2"));
}

#[test]
fn unset_endpoint_traces_without_buffering() {
    let shipper = LogShipper::new(config("api", "")).unwrap();
    shipper.record(LogEvent::error("GET /", "boom"));
    assert!(buffered_events(&shipper).is_empty());
}

#[test]
fn capacity_drops_the_oldest_event() {
    let mut config = config("api", "https://logs.example");
    config.capacity = 2;
    let shipper = LogShipper::new(config).unwrap();
    shipper.record(LogEvent::info("a"));
    shipper.record(LogEvent::info("b"));
    shipper.record(LogEvent::info("c"));
    let actions: Vec<_> = buffered_events(&shipper)
        .into_iter()
        .map(|event| event.action)
        .collect();
    assert_eq!(actions, [CAPACITY_WARN_NOTE.to_string(), "c".to_string()]);
}

#[test]
fn record_keeps_long_action_request_id_and_context() {
    let shipper = buffered();
    let mut event = LogEvent::info(format!("GET /{}", "p".repeat(3000)));
    event.id = "i".repeat(200);
    event
        .context
        .insert("ip".to_string(), "1".repeat(80).into());
    event
        .context
        .insert("user_agent".to_string(), "é".repeat(2000).into());
    let action = event.action.clone();
    let id = event.id.clone();
    let context = event.context.clone();
    shipper.record(event);
    let stored = buffered_events(&shipper).remove(0);
    assert_eq!(stored.action, action);
    assert_eq!(stored.id, id);
    assert_eq!(stored.context, context);
}

#[test]
fn record_inserts_one_capacity_warning_at_80_percent() {
    let mut config = config("api", "https://logs.example");
    config.capacity = 5;
    let shipper = LogShipper::new(config).unwrap();
    let mut oldest = LogEvent::error("first", "boom");
    oldest.id = "id-1".to_string();
    oldest
        .context
        .insert("ip".to_string(), "203.0.113.9".to_string().into());
    oldest
        .context
        .insert("user_agent".to_string(), "agent".to_string().into());
    oldest.latency_ms = Some(9);
    oldest.extra.insert("k".to_string(), "v".into());
    oldest.ts = DateTime::parse_from_rfc3339("2020-01-01T08:00:00+08:00").unwrap();
    let ts = oldest.ts;
    shipper.record(oldest);
    shipper.record(LogEvent::info("second"));
    shipper.record(LogEvent::info("third"));
    let below = buffered_events(&shipper);
    assert_eq!(below.len(), 3);
    assert_eq!(capacity_warn_count(&below), 0);
    assert_eq!(below[0].level, LogLevel::Error);
    assert_eq!(below[0].action, "first");
    let before = Utc::now();
    shipper.record(LogEvent::info("fourth"));
    let after = Utc::now();
    let events = buffered_events(&shipper);
    assert_eq!(events.len(), 5);
    assert_eq!(events[0].level, LogLevel::Error);
    assert_eq!(events[0].action, "first");
    assert_eq!(events[0].ts, ts);
    assert_eq!(events[0].id, "id-1");
    assert_eq!(events[0].context["ip"], "203.0.113.9");
    assert_eq!(events[0].context["user_agent"], "agent");
    assert_eq!(events[0].latency_ms, Some(9));
    assert_eq!(events[0].extra.get("k"), Some(&"v".into()));
    assert_eq!(events[1].action, "second");
    assert_eq!(events[1].level, LogLevel::Info);
    assert_eq!(events[2].action, "third");
    assert_eq!(events[3].action, "fourth");
    assert_eq!(events[3].level, LogLevel::Info);
    assert_eq!(capacity_warn_count(&events), 1);
    assert_capacity_warn(&events[4]);
    assert!(events[4].ts >= before && events[4].ts <= after);
    assert_eq!(events[4].ts.offset().local_minus_utc(), 8 * 3600);
    shipper.record(LogEvent::info("fifth"));
    let events = buffered_events(&shipper);
    assert_eq!(
        actions_of(&events),
        ["second", "third", "fourth", CAPACITY_WARN_NOTE, "fifth"]
    );
    assert_eq!(capacity_warn_count(&events), 1);
    shipper.record(LogEvent::info("sixth"));
    let events = buffered_events(&shipper);
    assert_eq!(
        actions_of(&events),
        ["third", "fourth", CAPACITY_WARN_NOTE, "fifth", "sixth"]
    );
    assert_eq!(capacity_warn_count(&events), 1);
}

#[test]
fn record_drops_the_oldest_event_to_store_the_capacity_warning() {
    let mut config = config("api", "https://logs.example");
    config.capacity = 4;
    let shipper = LogShipper::new(config).unwrap();
    for action in ["a", "b", "c"] {
        shipper.record(LogEvent::info(action));
    }
    let below = buffered_events(&shipper);
    assert_eq!(actions_of(&below), ["a", "b", "c"]);
    assert_eq!(capacity_warn_count(&below), 0);
    shipper.record(LogEvent::info("d"));
    let events = buffered_events(&shipper);
    assert_eq!(actions_of(&events), ["b", "c", "d", CAPACITY_WARN_NOTE]);
    assert_eq!(events[0].level, LogLevel::Info);
    assert_eq!(events[1].level, LogLevel::Info);
    assert_eq!(events[2].level, LogLevel::Info);
    assert_capacity_warn(&events[3]);
    shipper.record(LogEvent::info("e"));
    let events = buffered_events(&shipper);
    assert_eq!(actions_of(&events), ["c", "d", CAPACITY_WARN_NOTE, "e"]);
    assert_eq!(capacity_warn_count(&events), 1);
}

#[test]
fn record_at_capacity_one_stores_only_the_capacity_warning() {
    let mut config = config("api", "https://logs.example");
    config.capacity = 1;
    let shipper = LogShipper::new(config).unwrap();
    shipper.record(LogEvent::info("GET /"));
    let events = buffered_events(&shipper);
    assert_eq!(events.len(), 1);
    assert_capacity_warn(&events[0]);
    shipper.record(LogEvent::info("next"));
    let events = buffered_events(&shipper);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].action, "next");
    assert_eq!(events[0].level, LogLevel::Info);
    assert_eq!(capacity_warn_count(&events), 0);
}

fn trace_only() -> LogShipper {
    LogShipper::new(config("api", "")).unwrap()
}

fn parse_body(body: &str) -> Vec<LogEvent> {
    serde_json::from_str(body).unwrap()
}

#[tokio::test]
async fn unset_endpoint_ships_without_http() {
    let shipper = trace_only();
    shipper.record(LogEvent::info("local"));
    shipper.ship_batch().await;
    assert_eq!(shipper.buffered_len(), 0);
    assert!(shipper.inner.http.is_none());
}

#[tokio::test]
async fn empty_batch_does_not_post() {
    let config = config("api", "http://127.0.0.1:9");
    let shipper = LogShipper::new(config).unwrap();
    shipper.ship_batch().await;
    assert_eq!(shipper.buffered_len(), 0);
}

#[tokio::test]
async fn unreachable_endpoint_keeps_the_buffer() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let config = config("api", format!("http://{addr}"));
    let shipper = LogShipper::new(config).unwrap();
    shipper.record(LogEvent::error("down", "boom"));
    shipper.ship_batch().await;
    assert_eq!(shipper.buffered_len(), 1);
    assert_eq!(buffered_events(&shipper)[0].action, "down");
}

#[tokio::test]
async fn ship_batch_posts_json_to_the_app_path() {
    let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
    let config = config("my-app", endpoint);
    let shipper = LogShipper::new(config).unwrap();
    shipper.record(LogEvent::info("one"));
    shipper.record(LogEvent::warn("two"));
    shipper.record(LogEvent::error("three", "boom"));
    shipper.ship_batch().await;
    assert_eq!(shipper.buffered_len(), 0);
    let captured = rx.recv().await.unwrap();
    assert_eq!(captured.app, "my-app");
    assert_eq!(captured.path, "/log/my-app");
    assert_eq!(captured.content_type, "application/json");
    assert_eq!(captured.authorization, None);
    let events = parse_body(&captured.body);
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].action, "one");
    assert_eq!(events[1].action, "two");
    assert_eq!(events[2].action, "three");
    let body: serde_json::Value = serde_json::from_str(&captured.body).unwrap();
    let items = body.as_array().unwrap();
    assert!(body.as_object().is_none());
    assert_eq!(items.len(), 3);
    for item in items {
        assert!(item.get("app").is_none());
        assert!(item.get("project").is_none());
        assert!(item.get("action").is_some());
        assert!(item.get("request").is_none());
    }
    shipper.ship_batch().await;
    assert_eq!(shipper.buffered_len(), 0);
}

#[tokio::test]
async fn ship_batch_sends_the_post_api_key_as_a_bearer_token() {
    let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
    let mut config = config("api", endpoint);
    config.post_api_key = Some("  log-post-key  ".to_string());
    let shipper = LogShipper::new(config).unwrap();
    shipper.record(LogEvent::info("one"));
    shipper.ship_batch().await;
    let captured = rx.recv().await.unwrap();
    assert_eq!(
        captured.authorization.as_deref(),
        Some("Bearer log-post-key")
    );
    assert_eq!(shipper.buffered_len(), 0);
}

#[tokio::test]
async fn blank_post_api_key_sends_no_authorization() {
    for post_api_key in [None, Some(""), Some("   ")] {
        let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
        let mut config = config("api", endpoint);
        config.post_api_key = post_api_key.map(str::to_string);
        let shipper = LogShipper::new(config).unwrap();
        assert!(shipper.inner.post_api_key.is_none());
        shipper.record(LogEvent::info("one"));
        shipper.ship_batch().await;
        let captured = rx.recv().await.unwrap();
        assert_eq!(captured.authorization, None);
    }
}

#[tokio::test]
async fn ship_batch_posts_one_chunk_and_leaves_the_rest() {
    let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
    let shipper = LogShipper::new(config("api", endpoint)).unwrap();
    let total = SHIP_CHUNK + 1;
    for index in 0..total {
        shipper.record(LogEvent::info(index.to_string()));
    }
    shipper.ship_batch().await;
    assert_eq!(shipper.buffered_len(), 1);
    let captured = rx.recv().await.unwrap();
    let events = parse_body(&captured.body);
    assert_eq!(events.len(), SHIP_CHUNK);
    assert_eq!(events[0].action, "0");
    assert_eq!(events[SHIP_CHUNK - 1].action, (SHIP_CHUNK - 1).to_string());
    assert_eq!(buffered_events(&shipper)[0].action, SHIP_CHUNK.to_string());
    shipper.ship_batch().await;
    assert_eq!(shipper.buffered_len(), 0);
    let rest = rx.recv().await.unwrap();
    let rest_events = parse_body(&rest.body);
    assert_eq!(rest_events.len(), 1);
    assert_eq!(rest_events[0].action, SHIP_CHUNK.to_string());
}

#[tokio::test]
async fn http_error_restores_the_batch() {
    let (endpoint, _rx) = capture_server(StatusAction::Unavailable).await;
    let shipper = LogShipper::new(config("api", endpoint)).unwrap();
    shipper.record(LogEvent::info("keep"));
    shipper.ship_batch().await;
    assert_eq!(buffered_events(&shipper)[0].action, "keep");
}

#[tokio::test]
async fn in_flight_records_stay_out_of_a_successful_batch() {
    let (endpoint, _rx) = capture_server(StatusAction::Slow).await;
    let mut config = config("api", endpoint);
    config.capacity = 2;
    let shipper = LogShipper::new(config).unwrap();
    shipper.record(LogEvent::info("a"));
    shipper.record(LogEvent::info("b"));
    let shipper_task = shipper.clone();
    let ship = tokio::spawn(async move { shipper_task.ship_batch().await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while shipper.buffered_len() != 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    shipper.record(LogEvent::info("c"));
    ship.await.unwrap();
    let left = buffered_events(&shipper);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].action, "c");
}

#[tokio::test]
async fn failed_restore_keeps_the_oldest_batch() {
    let (endpoint, _rx) = capture_server(StatusAction::SlowUnavailable).await;
    let mut config = config("api", endpoint);
    config.capacity = 2;
    let shipper = LogShipper::new(config).unwrap();
    shipper.record(LogEvent::info("a"));
    shipper.record(LogEvent::info("b"));
    let shipper_task = shipper.clone();
    let ship = tokio::spawn(async move { shipper_task.ship_batch().await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while shipper.buffered_len() != 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    shipper.record(LogEvent::info("c"));
    ship.await.unwrap();
    let left: Vec<_> = buffered_events(&shipper)
        .into_iter()
        .map(|event| event.action)
        .collect();
    assert_eq!(left, ["b".to_string(), CAPACITY_WARN_NOTE.to_string()]);
}

#[tokio::test]
async fn trace_only_loop_keeps_running() {
    let mut config = config("api", "");
    config.ship_every = Duration::from_millis(10);
    let shipper = LogShipper::new(config).unwrap();
    shipper.record(LogEvent::info("traced"));
    let handle = shipper.spawn_ship_loop(ShutdownReceiver::new());
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert!(!handle.is_finished());
    handle.abort();
    assert_eq!(shipper.buffered_len(), 0);
}

#[tokio::test]
async fn shutdown_flushes_every_chunk_and_ends_the_loop() {
    let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
    let shipper = LogShipper::new(config("api", endpoint)).unwrap();
    let shutdown = ShutdownReceiver::new();
    let handle = shipper.spawn_ship_loop(shutdown.clone());
    tokio::time::sleep(Duration::from_millis(20)).await;
    for index in 0..SHIP_CHUNK + 1 {
        shipper.record(LogEvent::info(index.to_string()));
    }
    shutdown.trigger();
    tokio::time::timeout(Duration::from_secs(2), handle)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(shipper.buffered_len(), 0);
    assert_eq!(parse_body(&rx.recv().await.unwrap().body).len(), SHIP_CHUNK);
    assert_eq!(parse_body(&rx.recv().await.unwrap().body).len(), 1);
}

#[tokio::test]
async fn shutdown_flush_stops_when_the_endpoint_fails() {
    let (endpoint, _rx) = capture_server(StatusAction::Unavailable).await;
    let shipper = LogShipper::new(config("api", endpoint)).unwrap();
    shipper.record(LogEvent::info("keep"));
    let shutdown = ShutdownReceiver::new();
    shutdown.trigger();
    let handle = shipper.spawn_ship_loop(shutdown);
    tokio::time::timeout(Duration::from_secs(2), handle)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(buffered_events(&shipper)[0].action, "keep");
}

#[tokio::test]
async fn failed_ship_does_not_repeat_the_capacity_warning() {
    let (endpoint, _rx) = capture_server(StatusAction::Unavailable).await;
    let mut config = config("api", endpoint);
    config.capacity = 5;
    let shipper = LogShipper::new(config).unwrap();
    for action in ["a", "b", "c", "d"] {
        shipper.record(LogEvent::info(action));
    }
    shipper.ship_batch().await;
    let events = buffered_events(&shipper);
    assert_eq!(
        actions_of(&events),
        ["a", "b", "c", "d", CAPACITY_WARN_NOTE]
    );
    assert_eq!(events[0].level, LogLevel::Info);
    assert_capacity_warn(&events[4]);
    shipper.record(LogEvent::info("e"));
    let events = buffered_events(&shipper);
    assert_eq!(
        actions_of(&events),
        ["b", "c", "d", CAPACITY_WARN_NOTE, "e"]
    );
    assert_eq!(capacity_warn_count(&events), 1);
    assert_eq!(events[0].action, "b");
    assert_eq!(events[0].level, LogLevel::Info);
}

#[tokio::test]
async fn successful_ship_below_80_percent_warns_on_the_next_crossing() {
    let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
    let mut config = config("api", endpoint);
    config.capacity = 5;
    let shipper = LogShipper::new(config).unwrap();
    for action in ["a", "b", "c", "d"] {
        shipper.record(LogEvent::info(action));
    }
    shipper.ship_batch().await;
    assert_eq!(shipper.buffered_len(), 0);
    let posted = parse_body(&rx.recv().await.unwrap().body);
    assert_eq!(
        actions_of(&posted),
        ["a", "b", "c", "d", CAPACITY_WARN_NOTE]
    );
    assert_eq!(posted[0].level, LogLevel::Info);
    assert_eq!(posted[3].action, "d");
    assert_capacity_warn(&posted[4]);
    for action in ["e", "f", "g"] {
        shipper.record(LogEvent::info(action));
    }
    let events = buffered_events(&shipper);
    assert_eq!(actions_of(&events), ["e", "f", "g"]);
    assert_eq!(capacity_warn_count(&events), 0);
    shipper.record(LogEvent::info("h"));
    let events = buffered_events(&shipper);
    assert_eq!(
        actions_of(&events),
        ["e", "f", "g", "h", CAPACITY_WARN_NOTE]
    );
    assert_eq!(events[0].level, LogLevel::Info);
    assert_capacity_warn(&events[4]);
    shipper.record(LogEvent::info("i"));
    let events = buffered_events(&shipper);
    assert_eq!(
        actions_of(&events),
        ["f", "g", "h", CAPACITY_WARN_NOTE, "i"]
    );
    assert_eq!(capacity_warn_count(&events), 1);
}

#[tokio::test]
async fn successful_ship_at_80_percent_does_not_warn_again() {
    let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
    let mut config = config("api", endpoint);
    config.capacity = 5_000;
    let shipper = LogShipper::new(config).unwrap();
    for index in 0..4_000 {
        shipper.record(LogEvent::info(index.to_string()));
    }
    let events = buffered_events(&shipper);
    assert_eq!(events.len(), 4_001);
    assert_eq!(events[0].action, "0");
    assert_eq!(events[0].level, LogLevel::Info);
    assert_eq!(capacity_warn_count(&events), 1);
    assert_capacity_warn(events.last().unwrap());
    for index in 4_000..4_999 {
        shipper.record(LogEvent::info(index.to_string()));
    }
    assert_eq!(shipper.buffered_len(), 5_000);
    assert_eq!(capacity_warn_count(&buffered_events(&shipper)), 1);
    shipper.ship_batch().await;
    assert_eq!(shipper.buffered_len(), 4_000);
    let posted = parse_body(&rx.recv().await.unwrap().body);
    assert_eq!(posted.len(), SHIP_CHUNK);
    assert_eq!(posted[0].action, "0");
    assert_eq!(posted[0].level, LogLevel::Info);
    assert_eq!(capacity_warn_count(&posted), 0);
    let left = buffered_events(&shipper);
    assert_eq!(left[0].action, "1000");
    assert_eq!(left[0].level, LogLevel::Info);
    assert_eq!(capacity_warn_count(&left), 1);
    shipper.record(LogEvent::info("more"));
    let left = buffered_events(&shipper);
    assert_eq!(left.len(), 4_001);
    assert_eq!(left[0].action, "1000");
    assert_eq!(left.last().unwrap().action, "more");
    assert_eq!(left.last().unwrap().level, LogLevel::Info);
    assert_eq!(capacity_warn_count(&left), 1);
    shipper.ship_batch().await;
    assert_eq!(shipper.buffered_len(), 3_001);
    let second = parse_body(&rx.recv().await.unwrap().body);
    assert_eq!(second[0].action, "1000");
    assert_eq!(capacity_warn_count(&second), 0);
    assert_eq!(capacity_warn_count(&buffered_events(&shipper)), 1);
    for index in 0..998 {
        shipper.record(LogEvent::info(format!("extra-{index}")));
    }
    assert_eq!(shipper.buffered_len(), 3_999);
    assert_eq!(capacity_warn_count(&buffered_events(&shipper)), 1);
    shipper.record(LogEvent::info("cross"));
    let events = buffered_events(&shipper);
    assert_eq!(events.len(), 4_001);
    assert_eq!(capacity_warn_count(&events), 2);
    assert_eq!(events[events.len() - 2].action, "cross");
    assert_eq!(events[events.len() - 2].level, LogLevel::Info);
    assert_capacity_warn(events.last().unwrap());
}

enum StatusAction {
    NoContent,
    Unavailable,
    Slow,
    SlowUnavailable,
}

struct Captured {
    app: String,
    path: String,
    content_type: String,
    authorization: Option<String>,
    body: String,
}

async fn capture_server(
    action: StatusAction,
) -> (String, tokio::sync::mpsc::UnboundedReceiver<Captured>) {
    use axum::extract::{Path, State};
    use axum::http::{HeaderMap, StatusCode, Uri};
    use axum::routing::post;
    use axum::Router;

    #[derive(Clone)]
    struct Capture {
        tx: tokio::sync::mpsc::UnboundedSender<Captured>,
        action: u8,
    }

    let action_code = match action {
        StatusAction::NoContent => 0,
        StatusAction::Unavailable => 1,
        StatusAction::Slow => 2,
        StatusAction::SlowUnavailable => 3,
    };
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let app = Router::new()
        .route(
            "/log/{app}",
            post(
                |State(capture): State<Capture>,
                 Path(app): Path<String>,
                 uri: Uri,
                 headers: HeaderMap,
                 body: String| async move {
                    if capture.action == 2 || capture.action == 3 {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                    let content_type = headers
                        .get(axum::http::header::CONTENT_TYPE)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("")
                        .to_string();
                    let authorization = headers
                        .get(axum::http::header::AUTHORIZATION)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_string);
                    let _ = capture.tx.send(Captured {
                        app,
                        path: uri.path().to_string(),
                        content_type,
                        authorization,
                        body,
                    });
                    if capture.action == 1 || capture.action == 3 {
                        StatusCode::SERVICE_UNAVAILABLE
                    } else {
                        StatusCode::NO_CONTENT
                    }
                },
            ),
        )
        .with_state(Capture {
            tx,
            action: action_code,
        });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), rx)
}
