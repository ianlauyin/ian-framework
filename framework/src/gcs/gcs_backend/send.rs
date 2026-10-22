use std::time::{Duration, Instant};

use crate::logging::LogLevel;

use super::super::{api_detail, truncate, GcsError, GcsErrorKind};
use super::{logged_error, GcsBackend, OpsRecord};

pub(super) struct Call {
    pub method: reqwest::Method,
    pub url: String,
    pub headers: Vec<(reqwest::header::HeaderName, String)>,
    pub json: Option<serde_json::Value>,
    pub bytes: Option<Vec<u8>>,
    pub extra_status: &'static [u16],
    pub timeout: Option<Duration>,
}

impl GcsBackend {
    pub(super) async fn send(&self, call: &Call) -> Result<reqwest::Response, GcsError> {
        let mut retried = false;
        loop {
            let token = self.access_token().await?;
            let mut request = self.inner.http.request(call.method.clone(), &call.url);
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
                    let error_text = logged_error(&err);
                    self.record_op(OpsRecord {
                        level: LogLevel::Warn,
                        message: "gcs request failed",
                        method: call.method.as_str(),
                        url: &call.url,
                        status: None,
                        latency_ms: Some(millis_since(started)),
                        extra: &[("error", &error_text)],
                    });
                    return Err(GcsError::with_source(
                        GcsErrorKind::Transport,
                        "GCS request failed",
                        err,
                    ));
                }
            };
            let status = response.status().as_u16();
            let latency_ms = millis_since(started);
            if status == 401 && !retried {
                retried = true;
                let _ = response.bytes().await;
                self.invalidate_token().await;
                self.record_op(OpsRecord {
                    level: LogLevel::Warn,
                    message: "gcs access token rejected",
                    method: call.method.as_str(),
                    url: &call.url,
                    status: Some(status),
                    latency_ms: Some(latency_ms),
                    extra: &[],
                });
                continue;
            }
            if let Some(level) = level_for_status(status) {
                self.record_op(OpsRecord {
                    level,
                    message: "gcs response",
                    method: call.method.as_str(),
                    url: &call.url,
                    status: Some(status),
                    latency_ms: Some(latency_ms),
                    extra: &[],
                });
            }
            if response.status().is_success() || call.extra_status.contains(&status) {
                return Ok(response);
            }
            return Err(error_from_response(response).await);
        }
    }
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

fn millis_since(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn level_for_status(status: u16) -> Option<LogLevel> {
    if status >= 500 {
        Some(LogLevel::Error)
    } else if status >= 400 {
        Some(LogLevel::Warn)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_status_levels_are_warn_and_error() {
        assert_eq!(level_for_status(200), None);
        assert_eq!(level_for_status(308), None);
        assert_eq!(level_for_status(404), Some(LogLevel::Warn));
        assert_eq!(level_for_status(503), Some(LogLevel::Error));
    }
}
