mod auth;
mod modify;
mod ops_log;
mod read;
mod setup;
mod signing;
mod transport;
mod upload;

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::logging::LogShipper;

use super::encode::{encode_object_id, encode_seg};
use super::{GcsError, GcsErrorKind, API_ROOT};

use auth::{Auth, CachedToken};
use ops_log::OpsRecord;
use transport::Call;

pub use read::{ListPage, ListQuery, ObjectReader};
pub use upload::PutOptions;

const TRANSFER_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Clone)]
pub struct GcsClient {
    inner: Arc<Inner>,
}

impl GcsClient {
    pub fn bucket(&self) -> &str {
        &self.inner.bucket
    }

    pub fn signer_email(&self) -> &str {
        &self.inner.email
    }
}

impl fmt::Debug for GcsClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GcsClient")
            .field("bucket", &self.inner.bucket)
            .field("email", &self.inner.email)
            .field("auth", &self.inner.auth)
            .finish_non_exhaustive()
    }
}

struct Inner {
    bucket: String,
    email: String,
    auth: Auth,
    http: reqwest::Client,
    token: tokio::sync::Mutex<Option<CachedToken>>,
    ops_log: std::sync::Mutex<Option<LogShipper>>,
}

fn lock_ops(
    slot: &std::sync::Mutex<Option<LogShipper>>,
) -> std::sync::MutexGuard<'_, Option<LogShipper>> {
    slot.lock().unwrap_or_else(|err| err.into_inner())
}

fn validate_object_name(name: &str) -> Result<(), GcsError> {
    if name.is_empty() || name.contains(['\r', '\n']) {
        return Err(GcsError::new(
            GcsErrorKind::Config,
            "object name is empty or contains a line break",
        ));
    }
    Ok(())
}

fn validate_metadata(metadata: &BTreeMap<String, String>) -> Result<(), GcsError> {
    for (key, value) in metadata {
        if key.is_empty() || key.contains(['\r', '\n']) || value.contains(['\r', '\n']) {
            return Err(GcsError::new(
                GcsErrorKind::Config,
                "metadata key is empty or metadata contains a line break",
            ));
        }
    }
    Ok(())
}

fn object_url(bucket_name: &str, name: &str) -> String {
    format!(
        "{API_ROOT}/b/{}/o/{}",
        encode_seg(bucket_name),
        encode_object_id(name),
    )
}

fn logged_error(err: &impl std::fmt::Display) -> String {
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

fn api_detail(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    if let Some(message) = value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(|m| m.as_str())
    {
        return Some(message.to_string());
    }
    if let Some(message) = value.get("error_description").and_then(|m| m.as_str()) {
        return Some(message.to_string());
    }
    None
}

fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_detail_reads_the_gcs_message() {
        let body = r#"{"error":{"code":404,"message":"No such object"}}"#;
        assert_eq!(api_detail(body).as_deref(), Some("No such object"));
    }

    #[test]
    fn object_url_encodes_slashes() {
        assert_eq!(
            object_url("buck", "a/c"),
            "https://storage.googleapis.com/storage/v1/b/buck/o/a%2Fc"
        );
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
}
