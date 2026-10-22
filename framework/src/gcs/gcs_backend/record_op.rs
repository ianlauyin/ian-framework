use std::collections::BTreeMap;

use chrono::Utc;

use crate::logging::{LogEvent, LogLevel};

use super::GcsBackend;

pub(crate) struct OpsRecord<'a> {
    pub level: LogLevel,
    pub message: &'a str,
    pub method: &'a str,
    pub url: &'a str,
    pub status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub extra: &'a [(&'a str, &'a str)],
}

pub(crate) fn record_op(backend: &GcsBackend, op: OpsRecord<'_>) {
    let Some(shipper) = backend.ops_shipper() else {
        return;
    };
    let mut event = LogEvent {
        ts: Utc::now(),
        id: String::new(),
        ip: String::new(),
        user_agent: String::new(),
        action: action(op.method, op.url),
        status: op.status,
        latency_ms: op.latency_ms,
        level: op.level,
        message: op.message.to_string(),
        extra: BTreeMap::new(),
    };
    for (key, value) in op.extra {
        if *key == "bucket" {
            continue;
        }
        event.extra.insert(
            (*key).to_string(),
            serde_json::Value::String((*value).to_string()),
        );
    }
    event.extra.insert(
        "bucket".to_string(),
        serde_json::Value::String(backend.bucket().to_string()),
    );
    shipper.record(event);
}

pub(crate) fn millis_since(started: std::time::Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

pub(crate) fn logged_error(err: &impl std::fmt::Display) -> String {
    let redacted = redact_query(&err.to_string());
    super::truncate(&redacted, 300).to_string()
}

fn redact_query(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut skip = false;
    for ch in text.chars() {
        if skip {
            if ch.is_whitespace() {
                skip = false;
                out.push(ch);
            }
            continue;
        }
        if ch == '?' {
            skip = true;
            continue;
        }
        out.push(ch);
    }
    out
}

pub(crate) fn level_for_status(status: u16) -> Option<LogLevel> {
    if status >= 500 {
        Some(LogLevel::Error)
    } else if status >= 400 {
        Some(LogLevel::Warn)
    } else {
        None
    }
}

fn action(method: &str, url: &str) -> String {
    if method.is_empty() {
        return String::new();
    }
    let path = path_only(url);
    let path = if path.is_empty() { "/" } else { path.as_str() };
    format!("{method} {path}")
}

pub(crate) fn path_only(url: &str) -> String {
    if let Ok(parsed) = reqwest::Url::parse(url) {
        return parsed.path().to_string();
    }
    match url.split_once('?') {
        Some((path, _)) => path.to_string(),
        None => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_only_omits_the_query() {
        assert_eq!(
            path_only(
                "https://storage.googleapis.com/upload/storage/v1/b/bucket/o?uploadType=resumable&upload_id=secret"
            ),
            "/upload/storage/v1/b/bucket/o"
        );
        assert_eq!(path_only("/obj?upload_id=secret"), "/obj");
    }

    #[test]
    fn logged_error_omits_the_query() {
        let text = logged_error(
            &"error sending request for url (https://storage.googleapis.com/o?upload_id=secret): connection reset",
        );
        assert!(!text.contains("secret"));
        assert!(!text.contains("upload_id"));
        assert!(text.contains("connection reset"));
    }

    #[test]
    fn http_status_levels_are_warn_and_error() {
        assert_eq!(level_for_status(200), None);
        assert_eq!(level_for_status(308), None);
        assert_eq!(level_for_status(404), Some(LogLevel::Warn));
        assert_eq!(level_for_status(503), Some(LogLevel::Error));
    }

    fn user_auth() -> super::super::Auth {
        super::super::Auth::User {
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            refresh_token: "refresh".to_string(),
            signer_email: "signer@example.iam.gserviceaccount.com".to_string(),
        }
    }

    #[tokio::test]
    async fn ops_log_writes_the_bucket_and_omits_the_query() {
        let backend = super::super::GcsBackend::new("my-bucket", user_auth())
            .await
            .unwrap();
        let shipper =
            crate::logging::LogShipper::new(crate::logging::ShipperConfig::new("storage", ""))
                .unwrap();
        backend.attach_ops_log(shipper.clone());
        record_op(
            &backend,
            OpsRecord {
                level: crate::logging::LogLevel::Info,
                message: "gcs response",
                method: "GET",
                url:
                    "https://storage.googleapis.com/storage/v1/b/my-bucket/o/file?upload_id=secret",
                status: Some(200),
                latency_ms: Some(3),
                extra: &[],
            },
        );
        let events = shipper.buffered_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "GET /storage/v1/b/my-bucket/o/file");
        assert_eq!(events[0].id, "");
        assert_eq!(events[0].ip, "");
        assert_eq!(events[0].user_agent, "");
        assert_eq!(events[0].status, Some(200));
        assert_eq!(events[0].latency_ms, Some(3));
        assert_eq!(
            events[0]
                .extra
                .get("bucket")
                .and_then(|value| value.as_str()),
            Some("my-bucket")
        );
        let line = serde_json::to_string(&events[0]).unwrap();
        let value: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert!(value.get("app").is_none());
        assert!(value.get("project").is_none());
        assert!(!line.contains("secret"));
        assert!(!line.contains("upload_id"));
    }

    #[tokio::test]
    async fn ops_log_without_a_shipper_does_not_panic() {
        let backend = super::super::GcsBackend::new("my-bucket", user_auth())
            .await
            .unwrap();
        record_op(
            &backend,
            OpsRecord {
                level: crate::logging::LogLevel::Warn,
                message: "gcs request failed",
                method: "GET",
                url: "https://storage.googleapis.com/storage/v1/b/my-bucket/o/file",
                status: None,
                latency_ms: None,
                extra: &[],
            },
        );
    }
}
