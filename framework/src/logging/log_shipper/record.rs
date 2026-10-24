use crate::logging::{event, LogEvent, LogLevel};

use super::{meets_capacity_warn, LogShipper, CAPACITY_WARN_NOTE};

impl LogShipper {
    pub fn record(&self, event: LogEvent) {
        let capacity = self.inner.capacity;
        let mut buffer = self.lock_buffer();
        while buffer.events.len() >= capacity {
            buffer.events.pop_front();
        }
        buffer.events.push_back(event);
        if !buffer.capacity_warned && meets_capacity_warn(buffer.events.len(), capacity) {
            while buffer.events.len() >= capacity {
                buffer.events.pop_front();
            }
            buffer.events.push_back(capacity_warn_event());
            buffer.capacity_warned = true;
        }
    }
}

fn capacity_warn_event() -> LogEvent {
    event("", CAPACITY_WARN_NOTE, LogLevel::Warn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::log_shipper::CAPACITY_WARN_NOTE;
    use crate::logging::{LogShipper, ShipperConfig};
    use chrono::{DateTime, Utc};

    fn buffer_only() -> LogShipper {
        LogShipper::new(ShipperConfig::new("api", "")).unwrap()
    }

    fn messages_of(events: &[LogEvent]) -> Vec<&str> {
        events.iter().map(|event| event.message.as_str()).collect()
    }

    fn capacity_warn_count(events: &[LogEvent]) -> usize {
        events
            .iter()
            .filter(|event| event.message == CAPACITY_WARN_NOTE)
            .count()
    }

    fn assert_capacity_warn(event: &LogEvent) {
        assert_eq!(event.level, LogLevel::Warn);
        assert_eq!(event.message, CAPACITY_WARN_NOTE);
        assert_eq!(event.id, "");
        assert_eq!(event.ip, "");
        assert_eq!(event.user_agent, "");
        assert_eq!(event.action, "");
        assert_eq!(event.status, None);
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
        let shipper = buffer_only();
        shipper.record(event("", "kept", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].message, "kept");
        assert_eq!(shipper.inner.app, "api");
        assert_no_scope_keys(&serde_json::to_string(&events[0]).unwrap());
    }

    #[test]
    fn capacity_drops_the_oldest_event() {
        let mut config = ShipperConfig::new("api", "");
        config.capacity = 2;
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "a", LogLevel::Info));
        shipper.record(event("", "b", LogLevel::Info));
        shipper.record(event("", "c", LogLevel::Info));
        let messages: Vec<_> = shipper
            .buffered_events()
            .into_iter()
            .map(|event| event.message)
            .collect();
        assert_eq!(messages, [CAPACITY_WARN_NOTE.to_string(), "c".to_string()]);
    }

    #[test]
    fn record_keeps_long_message_request_id_ip_and_user_agent() {
        let shipper = buffer_only();
        let mut event = event(
            format!("GET /{}", "p".repeat(3000)),
            "é".repeat(5000),
            LogLevel::Info,
        );
        event.id = "i".repeat(200);
        event.ip = "1".repeat(80);
        event.user_agent = "é".repeat(2000);
        let message = event.message.clone();
        let action = event.action.clone();
        let id = event.id.clone();
        let ip = event.ip.clone();
        let user_agent = event.user_agent.clone();
        shipper.record(event);
        let stored = shipper.buffered_events().remove(0);
        assert_eq!(stored.message, message);
        assert_eq!(stored.action, action);
        assert_eq!(stored.id, id);
        assert_eq!(stored.ip, ip);
        assert_eq!(stored.user_agent, user_agent);
    }

    #[test]
    fn record_inserts_one_capacity_warning_at_80_percent() {
        let mut config = ShipperConfig::new("api", "");
        config.capacity = 5;
        let shipper = LogShipper::new(config).unwrap();
        let mut oldest = event("GET /a", "first", LogLevel::Error);
        oldest.id = "id-1".to_string();
        oldest.ip = "203.0.113.9".to_string();
        oldest.user_agent = "agent".to_string();
        oldest.status = Some(500);
        oldest.latency_ms = Some(9);
        oldest.extra.insert("k".to_string(), serde_json::json!("v"));
        oldest.ts = DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let ts = oldest.ts;
        shipper.record(oldest);
        shipper.record(event("", "second", LogLevel::Info));
        shipper.record(event("", "third", LogLevel::Info));
        let below = shipper.buffered_events();
        assert_eq!(below.len(), 3);
        assert_eq!(capacity_warn_count(&below), 0);
        assert_eq!(below[0].level, LogLevel::Error);
        assert_eq!(below[0].message, "first");
        let before = Utc::now();
        shipper.record(event("", "fourth", LogLevel::Info));
        let after = Utc::now();
        let events = shipper.buffered_events();
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].level, LogLevel::Error);
        assert_eq!(events[0].message, "first");
        assert_eq!(events[0].ts, ts);
        assert_eq!(events[0].id, "id-1");
        assert_eq!(events[0].ip, "203.0.113.9");
        assert_eq!(events[0].user_agent, "agent");
        assert_eq!(events[0].action, "GET /a");
        assert_eq!(events[0].status, Some(500));
        assert_eq!(events[0].latency_ms, Some(9));
        assert_eq!(
            events[0].extra.get("k").and_then(|value| value.as_str()),
            Some("v")
        );
        assert_eq!(events[1].message, "second");
        assert_eq!(events[1].level, LogLevel::Info);
        assert_eq!(events[2].message, "third");
        assert_eq!(events[3].message, "fourth");
        assert_eq!(events[3].level, LogLevel::Info);
        assert_eq!(capacity_warn_count(&events), 1);
        assert_capacity_warn(&events[4]);
        assert!(events[4].ts >= before && events[4].ts <= after);
        shipper.record(event("", "fifth", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(
            messages_of(&events),
            ["second", "third", "fourth", CAPACITY_WARN_NOTE, "fifth"]
        );
        assert_eq!(capacity_warn_count(&events), 1);
        shipper.record(event("", "sixth", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(
            messages_of(&events),
            ["third", "fourth", CAPACITY_WARN_NOTE, "fifth", "sixth"]
        );
        assert_eq!(capacity_warn_count(&events), 1);
    }

    #[test]
    fn record_drops_the_oldest_event_to_store_the_capacity_warning() {
        let mut config = ShipperConfig::new("api", "");
        config.capacity = 4;
        let shipper = LogShipper::new(config).unwrap();
        for message in ["a", "b", "c"] {
            shipper.record(event("", message, LogLevel::Info));
        }
        let below = shipper.buffered_events();
        assert_eq!(messages_of(&below), ["a", "b", "c"]);
        assert_eq!(capacity_warn_count(&below), 0);
        shipper.record(event("", "d", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(messages_of(&events), ["b", "c", "d", CAPACITY_WARN_NOTE]);
        assert_eq!(events[0].level, LogLevel::Info);
        assert_eq!(events[1].level, LogLevel::Info);
        assert_eq!(events[2].level, LogLevel::Info);
        assert_capacity_warn(&events[3]);
        shipper.record(event("", "e", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(messages_of(&events), ["c", "d", CAPACITY_WARN_NOTE, "e"]);
        assert_eq!(capacity_warn_count(&events), 1);
    }

    #[test]
    fn record_at_capacity_one_stores_only_the_capacity_warning() {
        let mut config = ShipperConfig::new("api", "");
        config.capacity = 1;
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("GET /", "hello", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(events.len(), 1);
        assert_capacity_warn(&events[0]);
        shipper.record(event("", "next", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].message, "next");
        assert_eq!(events[0].level, LogLevel::Info);
        assert_eq!(capacity_warn_count(&events), 0);
    }
}
