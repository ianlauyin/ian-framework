use std::collections::BTreeMap;

use chrono::Utc;

use crate::logging::{LogEvent, LogLevel};

use super::{lock_ops, GcsBackend};

pub(super) struct OpsRecord<'a> {
    pub level: LogLevel,
    pub message: &'a str,
    pub method: &'a str,
    pub url: &'a str,
    pub status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub extra: &'a [(&'a str, &'a str)],
}

impl GcsBackend {
    pub(super) fn record_op(&self, op: OpsRecord<'_>) {
        let Some(shipper) = lock_ops(&self.inner.ops_log).clone() else {
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
            serde_json::Value::String(self.bucket().to_string()),
        );
        shipper.record(event);
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

fn path_only(url: &str) -> String {
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
    use super::super::super::cred::Auth;
    use super::*;
    use crate::logging::{LogShipper, ShipperConfig};

    fn user_auth() -> Auth {
        Auth::User {
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            refresh_token: "refresh".to_string(),
            signer_email: "signer@example.iam.gserviceaccount.com".to_string(),
        }
    }

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

    #[tokio::test]
    async fn ops_log_writes_the_bucket_and_omits_the_query() {
        let backend = GcsBackend::new("my-bucket", user_auth()).await.unwrap();
        let shipper = LogShipper::new(ShipperConfig::new("storage", "")).unwrap();
        backend.attach_ops_log(shipper.clone());
        backend.record_op(OpsRecord {
            level: LogLevel::Info,
            message: "gcs response",
            method: "GET",
            url: "https://storage.googleapis.com/storage/v1/b/my-bucket/o/file?upload_id=secret",
            status: Some(200),
            latency_ms: Some(3),
            extra: &[],
        });
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
        let backend = GcsBackend::new("my-bucket", user_auth()).await.unwrap();
        backend.record_op(OpsRecord {
            level: LogLevel::Warn,
            message: "gcs request failed",
            method: "GET",
            url: "https://storage.googleapis.com/storage/v1/b/my-bucket/o/file",
            status: None,
            latency_ms: None,
            extra: &[],
        });
    }
}
