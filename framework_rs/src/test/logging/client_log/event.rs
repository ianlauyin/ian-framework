use axum::http::HeaderMap;

use crate::logging::log_shipper::SHIP_CHUNK;
use crate::logging::{ClientLog, LogEvent, LogLevel};
use crate::test::logging::{assert_uuid_v7, client_log_event};

#[test]
fn log_events_rejects_an_empty_list() {
    assert!(ClientLog { events: vec![] }
        .log_events(&HeaderMap::new())
        .is_none());
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
    assert_eq!(stored[0].context["ip"], "");
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
    assert!(ClientLog { events }.log_events(&HeaderMap::new()).is_none());
}
