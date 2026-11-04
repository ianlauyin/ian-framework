use std::collections::BTreeMap;

use crate::logging::{LogEvent, LogLevel};
use crate::time::hkt_now;

use crate::gcp::truncate;
use crate::gcs::redact_query;

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
            action: action(self.method, self.url),
            latency_ms: self.latency_ms,
            level: self.level,
            error_message: (self.level == LogLevel::Error).then(|| match self.status {
                Some(status) => format!("{} (status {status})", self.message),
                None => self.message.to_string(),
            }),
            context: BTreeMap::new(),
            info: serde_json::Map::new(),
        };
        event
            .info
            .insert("message".to_string(), self.message.to_string().into());
        if let Some(status) = self.status {
            event
                .info
                .insert("status".to_string(), status.to_string().into());
        }
        for (key, value) in self.extra {
            event
                .info
                .insert((*key).to_string(), (*value).to_string().into());
        }
        event
            .info
            .insert("bucket".to_string(), bucket.to_string().into());
        event
    }
}

pub(crate) fn logged_error(err: &impl std::fmt::Display) -> String {
    let redacted = redact_query(&err.to_string());
    truncate(&redacted, 300).to_string()
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
