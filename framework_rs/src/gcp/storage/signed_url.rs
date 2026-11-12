use std::time::Duration;

use crate::exception;
use crate::exception::Exception;
use crate::gcp::storage::encode::percent_encode;

const MAX_SIGNED_URL_SECONDS: u64 = 604_800;

pub(crate) fn validate_lifetime(expires_in: Duration) -> Result<Duration, Exception> {
    let seconds = expires_in.as_secs();
    if seconds == 0 {
        return Err(exception!(
            code = "GCP_CONFIG",
            "signed URL lifetime must be at least 1 second",
        ));
    }
    if seconds > MAX_SIGNED_URL_SECONDS {
        return Err(exception!(
            code = "GCP_CONFIG",
            "signed URL lifetime must be at most 604800 seconds",
        ));
    }
    Ok(Duration::from_secs(seconds))
}

pub(crate) fn content_disposition_value(filename: &str) -> Result<String, Exception> {
    if filename.is_empty() {
        return Err(exception!(
            code = "GCP_CONFIG",
            "attachment filename is empty",
        ));
    }
    let fallback: String = filename
        .chars()
        .map(|ch| {
            if ch.is_ascii() && !ch.is_ascii_control() && ch != '"' && ch != '\\' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    Ok(format!(
        "attachment; filename=\"{fallback}\"; filename*=UTF-8''{}",
        percent_encode(filename)
    ))
}
