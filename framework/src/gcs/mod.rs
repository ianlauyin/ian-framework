mod cred;
mod encode;
mod error;
mod gcs_backend;
mod object;
mod object_reader;

use std::time::Duration;

pub use encode::{encode_object_id, encode_object_path, encode_seg};
pub use error::{GcsError, GcsErrorKind};
pub use gcs_backend::GcsBackend;
pub use object::{ListPage, ListQuery, ObjectMeta, PutOptions};
pub use object_reader::ObjectReader;

const API_ROOT: &str = "https://storage.googleapis.com/storage/v1";
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(180);

fn validate_signer_email(email: &str) -> Result<(), GcsError> {
    if email.is_empty()
        || !email.contains('@')
        || email.contains(char::is_whitespace)
        || email.contains(['\r', '\n'])
    {
        return Err(GcsError::new(
            GcsErrorKind::Config,
            "signer email is not a service account email",
        ));
    }
    Ok(())
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
    fn transfer_timeout_is_180_seconds() {
        assert_eq!(TRANSFER_TIMEOUT, Duration::from_secs(180));
    }
}
