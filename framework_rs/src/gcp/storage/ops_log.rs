use crate::logging::{LogEvent, LogLevel};

use crate::gcp::storage::redact_query;
use crate::gcp::truncate;

pub(crate) struct OpsRecord<'a> {
    pub(crate) level: LogLevel,
    pub(crate) message: &'a str,
    pub(crate) method: &'a str,
    pub(crate) url: &'a str,
    pub(crate) status: Option<u16>,
    pub(crate) latency_ms: u64,
    pub(crate) extra: &'a [(&'a str, &'a str)],
}

impl OpsRecord<'_> {
    pub(crate) fn into_event(self, bucket: &str) -> LogEvent {
        let error_message = (self.level == LogLevel::Error).then(|| match self.status {
            Some(status) => format!("{} (status {status})", self.message),
            None => self.message.to_string(),
        });
        let mut event = LogEvent::build(action(self.method, self.url), self.level, error_message);
        event.latency_ms = self.latency_ms;
        event
            .extra
            .insert("message".to_string(), self.message.to_string().into());
        if let Some(status) = self.status {
            event
                .extra
                .insert("status".to_string(), status.to_string().into());
        }
        for (key, value) in self.extra {
            event
                .extra
                .insert((*key).to_string(), (*value).to_string().into());
        }
        event
            .extra
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

pub(crate) fn path_only(url: &str) -> String {
    if let Ok(parsed) = reqwest::Url::parse(url) {
        return parsed.path().to_string();
    }
    match url.split_once('?') {
        Some((path, _)) => path.to_string(),
        None => url.to_string(),
    }
}
