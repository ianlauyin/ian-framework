use crate::logging::{event, record, LogLevel};

use super::{bucket, ops_shipper, GcsBackend};

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
    let Some(shipper) = ops_shipper(backend) else {
        return;
    };
    let mut event = event("", op.level, op.message);
    event.method = op.method.to_string();
    event.path = path_only(op.url);
    event.status = op.status;
    event.latency_ms = op.latency_ms;
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
        serde_json::Value::String(bucket(backend).to_string()),
    );
    let _ = record(&shipper, event);
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

pub(crate) fn level_for_status(status: u16) -> LogLevel {
    if status >= 500 {
        LogLevel::Error
    } else if status >= 400 {
        LogLevel::Warn
    } else {
        LogLevel::Debug
    }
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
    fn http_status_levels_are_debug_warn_and_error() {
        assert_eq!(level_for_status(200), LogLevel::Debug);
        assert_eq!(level_for_status(308), LogLevel::Debug);
        assert_eq!(level_for_status(404), LogLevel::Warn);
        assert_eq!(level_for_status(503), LogLevel::Error);
    }
}
