use serde_json::Value;

use crate::logging::{event, LogEvent, LogLevel};

use super::{meets_capacity_warn, LogShipper, CAPACITY_WARN_NOTE};

impl LogShipper {
    pub fn record(&self, mut event: LogEvent) {
        mask_event(&self.inner.masks, &mut event);
        if self.inner.endpoint.is_none() {
            trace_event(&self.inner.app, &event);
            return;
        }
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

const MASK: &str = "***";

fn mask_event(masks: &[regex::Regex], event: &mut LogEvent) {
    if masks.is_empty() {
        return;
    }
    for (key, value) in event.extra.iter_mut() {
        mask_entry(masks, key, value);
    }
}

fn mask_entry(masks: &[regex::Regex], key: &str, value: &mut Value) {
    if masks.iter().any(|mask| mask.is_match(key)) {
        *value = Value::String(MASK.to_string());
    } else {
        mask_value(masks, value);
    }
}

fn mask_value(masks: &[regex::Regex], value: &mut Value) {
    match value {
        Value::String(text) => mask_text(masks, text),
        Value::Array(items) => items.iter_mut().for_each(|item| mask_value(masks, item)),
        Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                mask_entry(masks, key, item);
            }
        }
        _ => {}
    }
}

fn mask_text(masks: &[regex::Regex], text: &mut String) {
    for mask in masks {
        if let std::borrow::Cow::Owned(masked) = mask.replace_all(text, MASK) {
            *text = masked;
        }
    }
}

fn capacity_warn_event() -> LogEvent {
    event("", CAPACITY_WARN_NOTE, LogLevel::Warn)
}

fn trace_event(app: &str, event: &LogEvent) {
    let extra = serde_json::to_string(&event.extra).unwrap_or_default();
    macro_rules! emit {
        ($level:ident) => {
            tracing::$level!(
                app,
                id = %event.id,
                ip = %event.ip,
                user_agent = %event.user_agent,
                action = %event.action,
                status = ?event.status,
                latency_ms = ?event.latency_ms,
                extra = %extra,
                "{}",
                event.message
            )
        };
    }
    match event.level {
        LogLevel::Info => emit!(info),
        LogLevel::Warn => emit!(warn),
        LogLevel::Error => emit!(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::log_shipper::CAPACITY_WARN_NOTE;
    use crate::logging::{LogShipper, ShipperConfig};
    use chrono::{DateTime, Utc};

    fn config(endpoint: &str) -> ShipperConfig {
        let mut config = ShipperConfig::new("api");
        config.endpoint = Some(endpoint.to_string());
        config
    }

    fn buffered() -> LogShipper {
        LogShipper::new(config("http://logs.example")).unwrap()
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
        let shipper = buffered();
        shipper.record(event("", "kept", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].message, "kept");
        assert_eq!(shipper.inner.app, "api");
        assert_no_scope_keys(&serde_json::to_string(&events[0]).unwrap());
    }

    #[test]
    fn record_masks_every_pattern_match_in_extra_values() {
        let mut config = config("http://logs.example");
        config.mask_patterns = vec![
            r"password=\S+".to_string(),
            r"(?i)bearer [A-Za-z0-9._-]+".to_string(),
        ];
        let shipper = LogShipper::new(config).unwrap();
        let mut event = event(
            "POST /login?password=x",
            "login password=hunter2 ok",
            LogLevel::Info,
        );
        event.id = "password=1".to_string();
        event.user_agent = "agent BEARER abc.def".to_string();
        event
            .extra
            .insert("password=k".to_string(), serde_json::json!("Bearer t0k"));
        event.extra.insert(
            "note".to_string(),
            serde_json::json!("user password=a, BEARER b.c done"),
        );
        event.extra.insert(
            "nested".to_string(),
            serde_json::json!({"list": ["password=a", 7], "keep": "plain"}),
        );
        shipper.record(event);
        let stored = shipper.buffered_events().remove(0);
        assert_eq!(stored.message, "login password=hunter2 ok");
        assert_eq!(stored.action, "POST /login?password=x");
        assert_eq!(stored.id, "password=1");
        assert_eq!(stored.user_agent, "agent BEARER abc.def");
        assert_eq!(stored.extra["password=k"], serde_json::json!("***"));
        assert_eq!(stored.extra["note"], serde_json::json!("user *** *** done"));
        assert_eq!(
            stored.extra["nested"],
            serde_json::json!({"list": ["***", 7], "keep": "plain"})
        );
    }

    #[test]
    fn record_masks_the_value_of_a_matching_extra_key() {
        let mut config = config("http://logs.example");
        config.mask_patterns = vec![r"(?i)^(password|token)$".to_string()];
        let shipper = LogShipper::new(config).unwrap();
        let mut event = event("", "login", LogLevel::Info);
        event
            .extra
            .insert("password".to_string(), serde_json::json!("hunter2"));
        event
            .extra
            .insert("TOKEN".to_string(), serde_json::json!(1234));
        event.extra.insert(
            "nested".to_string(),
            serde_json::json!({"token": {"a": 1}, "user": "ian"}),
        );
        event
            .extra
            .insert("user".to_string(), serde_json::json!("ian"));
        shipper.record(event);
        let stored = shipper.buffered_events().remove(0);
        assert_eq!(stored.extra["password"], serde_json::json!("***"));
        assert_eq!(stored.extra["TOKEN"], serde_json::json!("***"));
        assert_eq!(
            stored.extra["nested"],
            serde_json::json!({"token": "***", "user": "ian"})
        );
        assert_eq!(stored.extra["user"], serde_json::json!("ian"));
    }

    #[test]
    fn record_without_mask_patterns_keeps_the_text() {
        let mut config = ShipperConfig::new("api");
        config.endpoint = Some("http://logs.example".to_string());
        config.mask_patterns = Vec::new();
        let shipper = LogShipper::new(config).unwrap();
        let mut event = event("", "password=hunter2", LogLevel::Info);
        event
            .extra
            .insert("note".to_string(), serde_json::json!("password=hunter2"));
        shipper.record(event);
        let stored = shipper.buffered_events().remove(0);
        assert_eq!(stored.message, "password=hunter2");
        assert_eq!(stored.extra["note"], serde_json::json!("password=hunter2"));
    }

    #[test]
    fn unset_endpoint_traces_without_buffering() {
        let shipper = LogShipper::new(config("")).unwrap();
        shipper.record(event("GET /", "traced", LogLevel::Error));
        assert!(shipper.buffered_events().is_empty());
    }

    #[test]
    fn capacity_drops_the_oldest_event() {
        let mut config = config("http://logs.example");
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
        let shipper = buffered();
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
        let mut config = config("http://logs.example");
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
        let mut config = config("http://logs.example");
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
        let mut config = config("http://logs.example");
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
