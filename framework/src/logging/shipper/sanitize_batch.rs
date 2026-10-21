use serde::{Deserialize, Serialize};

use super::super::LogEvent;
use super::SHIP_CHUNK;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogBatch {
    pub events: Vec<LogEvent>,
}

impl LogBatch {
    pub fn sanitize_untrusted_client(&self) -> Option<Vec<LogEvent>> {
        if self.events.is_empty() || self.events.len() > SHIP_CHUNK {
            return None;
        }
        let mut events = Vec::with_capacity(self.events.len());
        for event in &self.events {
            events.push(event.sanitize_untrusted_client()?);
        }
        Some(events)
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::event::event;
    use super::super::SHIP_CHUNK;
    use super::*;
    use crate::logging::LogLevel;
    use chrono::{DateTime, Utc};
    use serde_json::{json, Value};

    fn sample() -> LogEvent {
        let mut event = event("GET /health", "request", LogLevel::Info);
        event.ts = DateTime::parse_from_rfc3339("2026-10-18T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        event.id = "req-1".to_string();
        event.ip = "203.0.113.8".to_string();
        event.user_agent = "demo-agent".to_string();
        event.status = Some(204);
        event.latency_ms = Some(12);
        event.extra.insert("region".to_string(), json!("asia"));
        event
    }

    #[test]
    fn log_batch_round_trips() {
        let events = vec![sample(), event("", "down", LogLevel::Error)];
        let body = serde_json::to_string(&LogBatch {
            events: events.clone(),
        })
        .unwrap();
        let parsed: LogBatch = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed.events, events);
        let value: Value = serde_json::from_str(&body).unwrap();
        assert!(value.as_object().is_some());
        assert!(value["events"].is_array());
        assert!(serde_json::from_str::<Vec<LogEvent>>(&body).is_err());
    }

    #[test]
    fn sanitize_untrusted_client_rejects_an_empty_batch() {
        let body = LogBatch { events: vec![] };
        assert!(body.sanitize_untrusted_client().is_none());
    }

    #[test]
    fn sanitize_untrusted_client_rejects_the_batch_when_one_event_fails() {
        let body = LogBatch {
            events: vec![
                event("/ok", "kept", LogLevel::Info),
                event("/ok", "   ", LogLevel::Warn),
                event("/ok", "also", LogLevel::Error),
            ],
        };
        assert!(body.sanitize_untrusted_client().is_none());
    }

    #[test]
    fn sanitize_untrusted_client_returns_sanitized_events() {
        let body = LogBatch {
            events: vec![
                event("/a?x=1", " one ", LogLevel::Info),
                event("#frag", "two", LogLevel::Error),
            ],
        };
        let stored = body.sanitize_untrusted_client().unwrap();
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].action, "/a?x=1");
        assert_eq!(stored[0].message, "one");
        assert_eq!(stored[0].level, LogLevel::Info);
        assert_eq!(stored[0].id, "");
        assert_eq!(stored[0].ip, "");
        assert_eq!(stored[0].user_agent, "");
        assert_eq!(stored[0].status, None);
        assert_eq!(stored[0].latency_ms, None);
        assert_eq!(stored[1].action, "#frag");
        assert_eq!(stored[1].message, "two");
        assert_eq!(stored[1].level, LogLevel::Error);
    }

    #[test]
    fn sanitize_untrusted_client_accepts_a_batch_at_the_ship_chunk() {
        let body = LogBatch {
            events: (0..SHIP_CHUNK)
                .map(|_| event("/ok", "kept", LogLevel::Info))
                .collect(),
        };
        let stored = body.sanitize_untrusted_client().unwrap();
        assert_eq!(stored.len(), SHIP_CHUNK);
        assert_eq!(stored[0].message, "kept");
        assert_eq!(stored[0].id, "");
    }

    #[test]
    fn sanitize_untrusted_client_rejects_a_batch_above_the_ship_chunk() {
        let body = LogBatch {
            events: (0..=SHIP_CHUNK)
                .map(|_| event("/ok", "kept", LogLevel::Info))
                .collect(),
        };
        assert_eq!(body.events.len(), SHIP_CHUNK + 1);
        assert!(body.sanitize_untrusted_client().is_none());
    }
}
