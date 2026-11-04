use serde::{Deserialize, Serialize};

use super::{LogEvent, SHIP_CHUNK};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogBatch {
    pub events: Vec<LogEvent>,
}

impl LogBatch {
    pub fn events_opt(&self) -> Option<Vec<LogEvent>> {
        if self.events.is_empty() || self.events.len() > SHIP_CHUNK {
            return None;
        }
        Some(self.events.iter().map(LogEvent::from_client).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::LogLevel;
    use crate::logging::LogValue;
    use chrono::DateTime;
    use serde_json::{json, Value};

    fn sample() -> LogEvent {
        let mut event = LogEvent::info("GET /health");
        event.ts = DateTime::parse_from_rfc3339("2026-10-18T20:00:00+08:00").unwrap();
        event.id = "req-1".to_string();
        event
            .context
            .insert("ip".to_string(), LogValue::from("203.0.113.8"));
        event
            .context
            .insert("user_agent".to_string(), LogValue::from("demo-agent"));
        event.latency_ms = Some(12);
        event.extra.insert("region".to_string(), json!("asia"));
        event
    }

    #[test]
    fn log_batch_round_trips() {
        let down = LogEvent::error("down", "boom");
        let events = vec![sample(), down];
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
    fn events_opt_rejects_an_empty_batch() {
        let body = LogBatch { events: vec![] };
        assert!(body.events_opt().is_none());
    }

    #[test]
    fn events_opt_returns_sanitized_events() {
        let body = LogBatch {
            events: vec![LogEvent::info("/a?x=1"), LogEvent::error("#frag", "boom")],
        };
        let stored = body.events_opt().unwrap();
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].action, "/a?x=1");
        assert_eq!(stored[0].level, LogLevel::Info);
        assert_eq!(stored[0].id, "");
        assert!(stored[0].context.is_empty());
        assert_eq!(stored[0].latency_ms, None);
        assert_eq!(stored[1].action, "#frag");
        assert_eq!(stored[1].level, LogLevel::Error);
    }

    #[test]
    fn events_opt_accepts_a_batch_at_the_ship_chunk() {
        let body = LogBatch {
            events: (0..SHIP_CHUNK).map(|_| LogEvent::info("/ok")).collect(),
        };
        let stored = body.events_opt().unwrap();
        assert_eq!(stored.len(), SHIP_CHUNK);
        assert_eq!(stored[0].id, "");
    }

    #[test]
    fn events_opt_rejects_a_batch_above_the_ship_chunk() {
        let body = LogBatch {
            events: (0..=SHIP_CHUNK).map(|_| LogEvent::info("/ok")).collect(),
        };
        assert_eq!(body.events.len(), SHIP_CHUNK + 1);
        assert!(body.events_opt().is_none());
    }
}
