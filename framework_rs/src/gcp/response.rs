use http_body_util::{BodyExt, LengthLimitError, Limited};

use crate::exception;
use crate::exception::Exception;

// A Google API response or error body is far below this size. A larger body is an error.
pub(crate) const RESPONSE_BODY_LIMIT: usize = 256 * 1024;

pub(crate) fn google_error_message(body: &str) -> Option<String> {
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

pub(crate) fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.get(..end).unwrap_or_default()
}

pub(crate) fn is_retryable_status(status: u16) -> bool {
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504)
}

// `what` names the body in the error message, and `code` is the error code.
pub(crate) async fn read_limited(
    response: reqwest::Response,
    code: &'static str,
    what: &str,
) -> Result<Vec<u8>, Exception> {
    let body = reqwest::Body::from(response);
    let err = match Limited::new(body, RESPONSE_BODY_LIMIT).collect().await {
        Ok(collected) => return Ok(collected.to_bytes().into()),
        Err(err) => err,
    };
    if err.is::<LengthLimitError>() {
        return Err(exception!(
            code = code,
            format!("{what} is larger than 256 KiB"),
        ));
    }
    let message = format!("{what} is not readable");
    Err(match err.downcast::<reqwest::Error>() {
        Ok(err) => exception!(code = code, message, source = *err),
        Err(_) => exception!(code = code, message),
    })
}
