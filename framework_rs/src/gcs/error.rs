use crate::exception;
use crate::exception::Exception;
use crate::gcp::{api_detail, truncate};

const RESPONSE_DETAIL_MAX_BYTES: usize = 200;
const SIGNED_PARAM_PREFIX: &str = "x-goog-";
const REDACTED: &str = "***";
const SECRET_KEYS: [&str; 11] = [
    "upload_id",
    "access_token",
    "refresh_token",
    "id_token",
    "token",
    "signature",
    "sig",
    "client_secret",
    "password",
    "secret",
    "authorization",
];

pub(crate) fn is_retryable(err: &Exception) -> bool {
    matches!(err.code(), "GCS_TRANSPORT" | "GCS_UNAVAILABLE")
}

pub(crate) fn is_retryable_status(status: u16) -> bool {
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504)
}

// A URL or a path keeps its path and loses its query. A value of a known secret key is replaced. Other text stays.
pub(crate) fn redact_query(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut redact_next = false;
    for (is_token, part) in split_tokens(text) {
        if !is_token {
            out.push_str(part);
            continue;
        }
        let lower = part.to_ascii_lowercase();
        if redact_next && !matches!(lower.as_str(), "bearer" | "basic") {
            out.push_str(REDACTED);
            redact_next = false;
            continue;
        }
        redact_next = matches!(
            lower.trim_end_matches([':', '=']),
            "authorization" | "bearer" | "basic"
        );
        out.push_str(&redact_token(part));
    }
    out
}

fn split_tokens(text: &str) -> Vec<(bool, &str)> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_token = false;
    for (index, ch) in text.char_indices() {
        let separator = is_token_end(ch);
        if separator == in_token {
            if let Some(part) = text.get(start..index) {
                if !part.is_empty() {
                    parts.push((in_token, part));
                }
            }
            start = index;
            in_token = !separator;
        }
    }
    if let Some(part) = text.get(start..) {
        if !part.is_empty() {
            parts.push((in_token, part));
        }
    }
    parts
}

fn redact_token(token: &str) -> String {
    let token = match token.split_once('?') {
        Some((path, _)) if path.contains('/') => path,
        _ => token,
    };
    token
        .split('&')
        .map(|pair| match pair.split_once('=') {
            Some((key, _)) if is_secret_key(key) => format!("{key}={REDACTED}"),
            _ => pair.to_string(),
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn is_secret_key(key: &str) -> bool {
    let key = key.trim_start_matches('?').to_ascii_lowercase();
    key.starts_with(SIGNED_PARAM_PREFIX) || SECRET_KEYS.contains(&key.as_str())
}

fn is_token_end(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            '"' | '\'' | '<' | '>' | ')' | '\\' | ',' | ';' | ']' | '}'
        )
}

// The gcp code and the gcs code have the same meaning. The message and the source stay.
pub(crate) fn from_gcp(err: Exception) -> Exception {
    let code = match err.code() {
        "GCP_AUTH" => "GCS_AUTH",
        "GCP_CONFIG" => "GCS_CONFIG",
        "GCP_API" => "GCS_API",
        "GCP_TRANSPORT" => "GCS_TRANSPORT",
        _ => return err,
    };
    err.with_code(code)
}

pub(crate) async fn gcs_response_exception(response: reqwest::Response) -> Exception {
    let status = response.status().as_u16();
    let text = response.text().await.unwrap_or_default();
    let detail = response_detail(&text);
    let code = match status {
        401 | 403 => "GCS_AUTH",
        404 => "GCS_NOT_FOUND",
        _ if is_retryable_status(status) => "GCS_UNAVAILABLE",
        _ => "GCS_API",
    };
    exception!(format!("status {status}: {detail}"), code = code)
}

// The upstream body can hold a signed URL, so the detail has no URL query and no control character.
pub(crate) fn response_detail(body: &str) -> String {
    let detail = api_detail(body).unwrap_or_else(|| body.to_string());
    let redacted = redact_query(truncate(&detail, RESPONSE_DETAIL_MAX_BYTES * 10));
    let clean: String = redacted
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect();
    truncate(&clean, RESPONSE_DETAIL_MAX_BYTES).to_string()
}
