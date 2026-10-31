use std::collections::BTreeMap;

use crate::logging::{LogEvent, LogLevel};
use crate::time::hkt_now;

use crate::gcp::truncate;

pub(crate) struct OpsRecord<'a> {
    pub(crate) level: LogLevel,
    pub(crate) message: &'a str,
    pub(crate) method: &'a str,
    pub(crate) url: &'a str,
    pub(crate) status: Option<u16>,
    pub(crate) latency_ms: Option<u64>,
    pub(crate) extra: &'a [(&'a str, &'a str)],
}

impl OpsRecord<'_> {
    pub(crate) fn into_event(self, bucket: &str) -> LogEvent {
        let mut event = LogEvent {
            ts: hkt_now(),
            id: String::new(),
            ip: String::new(),
            user_agent: String::new(),
            action: action(self.method, self.url),
            latency_ms: self.latency_ms,
            level: self.level,
            extra: BTreeMap::new(),
        };
        event.extra.insert(
            "message".to_string(),
            serde_json::Value::String(self.message.to_string()),
        );
        if let Some(status) = self.status {
            event.extra.insert(
                "status".to_string(),
                serde_json::Value::String(status.to_string()),
            );
        }
        for (key, value) in self.extra {
            event.extra.insert(
                (*key).to_string(),
                serde_json::Value::String((*value).to_string()),
            );
        }
        event.extra.insert(
            "bucket".to_string(),
            serde_json::Value::String(bucket.to_string()),
        );
        event
    }
}

pub(crate) fn logged_error(err: &impl std::fmt::Display) -> String {
    let redacted = redact_query(&err.to_string());
    truncate(&redacted, 300).to_string()
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

fn action(method: &str, url: &str) -> String {
    if method.is_empty() {
        return String::new();
    }
    let path = path_only(url);
    let path = if path.is_empty() { "/" } else { path.as_str() };
    format!("GCS/{method}:{path}")
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
    use super::*;

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
    fn into_event_ts_has_the_hkt_offset() {
        let record = OpsRecord {
            level: LogLevel::Info,
            message: "uploaded",
            method: "PUT",
            url: "/obj",
            status: Some(200),
            latency_ms: Some(5),
            extra: &[],
        };
        let event = record.into_event("bucket");
        assert_eq!(event.ts.offset().local_minus_utc(), 8 * 3600);
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
}
