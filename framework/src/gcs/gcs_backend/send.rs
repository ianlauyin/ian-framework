use std::time::{Duration, Instant};

use super::{ops, token, GcsBackend, GcsError, GcsErrorKind};

pub(crate) const TRANSFER_TIMEOUT: Duration = Duration::from_secs(180);

pub(crate) struct Call {
    pub method: reqwest::Method,
    pub url: String,
    pub headers: Vec<(reqwest::header::HeaderName, String)>,
    pub json: Option<serde_json::Value>,
    pub bytes: Option<Vec<u8>>,
    pub extra_status: &'static [u16],
    pub timeout: Option<Duration>,
}

pub(crate) async fn send(backend: &GcsBackend, call: &Call) -> Result<reqwest::Response, GcsError> {
    let mut retried = false;
    loop {
        let token = token::access_token(backend).await?;
        let mut request = backend.http().request(call.method.clone(), &call.url);
        request = request.bearer_auth(&token);
        for (name, value) in &call.headers {
            request = request.header(name, value);
        }
        if let Some(timeout) = call.timeout {
            request = request.timeout(timeout);
        }
        if let Some(json) = &call.json {
            request = request.json(json);
        } else if let Some(bytes) = &call.bytes {
            request = request.body(bytes.clone());
        }
        let started = Instant::now();
        let response = match request.send().await {
            Ok(response) => response,
            Err(err) => {
                let error_text = ops::logged_error(&err);
                ops::record_op(
                    backend,
                    ops::OpsRecord {
                        level: crate::logging::LogLevel::Warn,
                        message: "gcs request failed",
                        method: call.method.as_str(),
                        url: &call.url,
                        status: None,
                        latency_ms: Some(ops::millis_since(started)),
                        extra: &[("error", &error_text)],
                    },
                );
                return Err(GcsError::with_source(
                    GcsErrorKind::Transport,
                    "GCS request failed",
                    err,
                ));
            }
        };
        let status = response.status().as_u16();
        let latency_ms = ops::millis_since(started);
        if status == 401 && !retried {
            retried = true;
            let _ = response.bytes().await;
            token::invalidate(backend).await;
            ops::record_op(
                backend,
                ops::OpsRecord {
                    level: crate::logging::LogLevel::Warn,
                    message: "gcs access token rejected",
                    method: call.method.as_str(),
                    url: &call.url,
                    status: Some(status),
                    latency_ms: Some(latency_ms),
                    extra: &[],
                },
            );
            continue;
        }
        if let Some(level) = ops::level_for_status(status) {
            ops::record_op(
                backend,
                ops::OpsRecord {
                    level,
                    message: "gcs response",
                    method: call.method.as_str(),
                    url: &call.url,
                    status: Some(status),
                    latency_ms: Some(latency_ms),
                    extra: &[],
                },
            );
        }
        if response.status().is_success() || call.extra_status.contains(&status) {
            return Ok(response);
        }
        return Err(error_from_response(response).await);
    }
}

pub(crate) fn api_detail(body: &str) -> Option<String> {
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
    &text[..end]
}

async fn error_from_response(response: reqwest::Response) -> GcsError {
    let status = response.status().as_u16();
    let text = response.text().await.unwrap_or_default();
    let detail = api_detail(&text).unwrap_or(text);
    let detail = truncate(&detail, 500);
    let kind = match status {
        401 | 403 => GcsErrorKind::Auth,
        404 => GcsErrorKind::NotFound,
        _ => GcsErrorKind::Api,
    };
    GcsError::with_status(kind, status, format!("status {status}: {detail}"))
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
