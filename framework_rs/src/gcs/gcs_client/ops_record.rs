use std::collections::BTreeMap;

use chrono::Utc;

use crate::logging::{LogEvent, LogLevel};

pub(super) struct OpsRecord<'a> {
    pub(super) level: LogLevel,
    pub(super) message: &'a str,
    pub(super) method: &'a str,
    pub(super) url: &'a str,
    pub(super) status: Option<u16>,
    pub(super) latency_ms: Option<u64>,
    pub(super) extra: &'a [(&'a str, &'a str)],
}

impl OpsRecord<'_> {
    pub(super) fn into_event(self, bucket: &str) -> LogEvent {
        let mut event = LogEvent {
            ts: Utc::now(),
            id: String::new(),
            ip: String::new(),
            user_agent: String::new(),
            action: action(self.method, self.url),
            status: self.status,
            latency_ms: self.latency_ms,
            level: self.level,
            message: self.message.to_string(),
            extra: BTreeMap::new(),
        };
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
}
