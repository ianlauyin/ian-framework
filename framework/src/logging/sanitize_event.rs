use std::collections::BTreeMap;

use chrono::Utc;
use serde_json::Value;

use super::LogEvent;

impl LogEvent {
    pub fn sanitize_untrusted_client(&self) -> Option<LogEvent> {
        let message = self.message.trim();
        if message.is_empty() {
            return None;
        }
        Some(LogEvent {
            ts: Utc::now(),
            id: String::new(),
            ip: String::new(),
            user_agent: String::new(),
            action: self.action.clone(),
            status: None,
            latency_ms: None,
            level: self.level,
            message: message.to_string(),
            extra: client_extra(&self.extra),
        })
    }
}

fn client_extra(extra: &BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    let mut kept = BTreeMap::new();
    for (key, value) in extra {
        if !extra_key_ok(key) {
            continue;
        }
        let Some(text) = extra_value(value) else {
            continue;
        };
        kept.insert(key.clone(), Value::String(text));
    }
    kept
}

fn extra_key_ok(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

fn extra_value(value: &Value) -> Option<String> {
    let text = match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        _ => return None,
    };
    if text.contains('\r') || text.contains('\n') {
        return None;
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::super::event::event;
    use chrono::{DateTime, Utc};
    use serde_json::json;

    #[test]
    fn sanitize_untrusted_client_drops_untrusted_fields() {
        let mut event = event(
            "GET /download?sig=abc#top",
            "  saved file  ",
            super::super::LogLevel::Warn,
        );
        event.ts = DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        event.id = "client-id".to_string();
        event.ip = "203.0.113.8".to_string();
        event.user_agent = "browser".to_string();
        event.status = Some(500);
        event.latency_ms = Some(42);
        let before = Utc::now();
        let stored = event.sanitize_untrusted_client().unwrap();
        let after = Utc::now();
        assert_eq!(event.id, "client-id");
        assert_eq!(stored.id, "");
        assert_eq!(stored.ip, "");
        assert_eq!(stored.user_agent, "");
        assert_eq!(stored.status, None);
        assert_eq!(stored.latency_ms, None);
        assert_eq!(stored.level, super::super::LogLevel::Warn);
        assert_eq!(stored.message, "saved file");
        assert_eq!(stored.action, "GET /download?sig=abc#top");
        assert_ne!(stored.ts, event.ts);
        assert!(stored.ts >= before && stored.ts <= after);
    }

    #[test]
    fn sanitize_untrusted_client_keeps_the_client_action() {
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
            let stored = event(action, "ok", super::super::LogLevel::Info)
                .sanitize_untrusted_client()
                .unwrap();
            assert_eq!(stored.action, action);
        }
    }

    #[test]
    fn sanitize_untrusted_client_filters_extra() {
        let mut client = event("/files", "note", super::super::LogLevel::Info);
        client.extra.insert("bad key".to_string(), json!("no"));
        client.extra.insert("k01".to_string(), json!("kept"));
        client.extra.insert("k02".to_string(), json!("a\nb"));
        client.extra.insert("k03".to_string(), json!("a\rb"));
        client.extra.insert("k04".to_string(), json!(null));
        client.extra.insert("k05".to_string(), json!({"a": 1}));
        client.extra.insert("k06".to_string(), json!([1]));
        client
            .extra
            .insert("k07".to_string(), json!("x".repeat(250)));
        client.extra.insert("k08".to_string(), json!("ninth"));
        client.extra.insert("k09".to_string(), json!(true));
        let stored = client.sanitize_untrusted_client().unwrap();
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
        assert_eq!(stored.extra["k09"], json!("true"));
        assert!(!stored.extra.contains_key("bad key"));

        let mut scalars = event("/files", "note", super::super::LogLevel::Error);
        scalars.extra.insert("a/b".to_string(), json!("no"));
        scalars.extra.insert("a_b-c.d".to_string(), json!("yes"));
        scalars.extra.insert("a".repeat(65), json!("long-key"));
        scalars.extra.insert("count".to_string(), json!(12));
        scalars.extra.insert("flag".to_string(), json!(true));
        scalars.extra.insert("off".to_string(), json!(false));
        scalars.extra.insert("ratio".to_string(), json!(1.5));
        let stored = scalars.sanitize_untrusted_client().unwrap();
        assert_eq!(stored.extra.get("a/b"), None);
        assert_eq!(stored.extra[&"a".repeat(65)], json!("long-key"));
        assert_eq!(stored.extra["a_b-c.d"], json!("yes"));
        assert_eq!(stored.extra["count"], json!("12"));
        assert_eq!(stored.extra["flag"], json!("true"));
        assert_eq!(stored.extra["off"], json!("false"));
        assert_eq!(stored.extra["ratio"], json!("1.5"));

        let mut text = event("/files", "note", super::super::LogLevel::Info);
        text.extra.insert(String::new(), json!("no"));
        text.extra.insert("café".to_string(), json!("no"));
        text.extra.insert("ok!".to_string(), json!("no"));
        let long = "中".repeat(67);
        text.extra.insert("text".to_string(), json!(long));
        let stored = text.sanitize_untrusted_client().unwrap();
        assert_eq!(stored.extra.get(""), None);
        assert_eq!(stored.extra.get("café"), None);
        assert_eq!(stored.extra.get("ok!"), None);
        assert_eq!(stored.extra["text"], json!(long));

        let mut later = event("/files", "note", super::super::LogLevel::Info);
        for index in 0..8 {
            later.extra.insert(format!("bad {index}"), json!("no"));
        }
        later.extra.insert("zzzz".to_string(), json!("yes"));
        let stored = later.sanitize_untrusted_client().unwrap();
        assert_eq!(stored.extra.len(), 1);
        assert_eq!(stored.extra["zzzz"], json!("yes"));
    }

    #[test]
    fn sanitize_untrusted_client_rejects_a_blank_message() {
        for message in ["", " ", "\n", "\t", " \r\n "] {
            assert!(
                event("/ok", message, super::super::LogLevel::Info)
                    .sanitize_untrusted_client()
                    .is_none(),
                "{message:?}"
            );
        }
    }
}
