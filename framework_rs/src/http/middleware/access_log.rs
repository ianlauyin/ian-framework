use std::collections::BTreeMap;
use std::time::Instant;

use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::http::headers::client::{client_ip, header_text, user_agent};
use crate::logging::{LogEvent, LogValue, Logger};

pub(crate) const BODY_LOG_LIMIT: usize = 64 * 1024;

pub async fn access_log(State(logger): State<Logger>, request: Request, next: Next) -> Response {
    let method = request.method().as_str().to_string();
    let path = request_path(request.uri().path());
    let ip = client_ip(request.headers());
    let user_agent = user_agent(request.headers());
    let request_url = request
        .uri()
        .path_and_query()
        .map_or_else(|| path.clone(), |url| url.as_str().to_string());
    let started = Instant::now();
    let (request, body) = split_json_body(request).await;
    let response = match request {
        Some(request) => next.run(request).await,
        None => StatusCode::BAD_REQUEST.into_response(),
    };
    let status = response.status().as_u16();
    let mut event = new_event(action(&method, &path), status);
    event.context.insert("ip".to_string(), ip.into());
    event
        .context
        .insert("user_agent".to_string(), user_agent.into());
    event
        .context
        .insert("request_url".to_string(), request_url.into());
    event.latency_ms = Some(millis_since(started));
    if let Some(fields) = body {
        event.extra = fields;
    }
    logger.record(event);
    response
}

async fn split_json_body(
    request: Request,
) -> (Option<Request>, Option<BTreeMap<String, LogValue>>) {
    if !is_small_json_body(request.headers()) {
        return (Some(request), None);
    }
    let (parts, body) = request.into_parts();
    let Ok(bytes) = to_bytes(body, BODY_LOG_LIMIT).await else {
        return (None, None);
    };
    let fields = serde_json::from_slice::<BTreeMap<String, LogValue>>(&bytes).ok();
    (Some(Request::from_parts(parts, Body::from(bytes))), fields)
}

// Only a body with a known small length is read, so a large or streamed body reaches the handler untouched.
fn is_small_json_body(headers: &HeaderMap) -> bool {
    let length = header_text(headers, header::CONTENT_LENGTH.as_str())
        .and_then(|length| length.parse::<usize>().ok());
    if !matches!(length, Some(1..=BODY_LOG_LIMIT)) {
        return false;
    }
    let Some(content_type) = header_text(headers, header::CONTENT_TYPE.as_str()) else {
        return false;
    };
    let mime = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    mime == "application/json" || mime.ends_with("+json")
}

fn action(method: &str, path: &str) -> String {
    if method.is_empty() {
        return String::new();
    }
    let path = if path.is_empty() { "/" } else { path };
    format!("API/{method}:{path}")
}

fn request_path(path: &str) -> String {
    if path.is_empty() {
        "/".to_string()
    } else {
        path.to_string()
    }
}

fn new_event(action: String, status: u16) -> LogEvent {
    if status >= 500 {
        LogEvent::error(action, format!("response status {status}"))
    } else if status >= 400 {
        LogEvent::warn(action)
    } else {
        LogEvent::info(action)
    }
}

fn millis_since(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
