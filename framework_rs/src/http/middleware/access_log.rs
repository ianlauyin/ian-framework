use std::time::Instant;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;

use crate::http::headers::client::{client_ip, user_agent};
use crate::logging::{LogEvent, Logger};

pub async fn access_log(State(logger): State<Logger>, request: Request, next: Next) -> Response {
    let method = request.method().as_str().to_string();
    let path = request_path(request.uri().path());
    let ip = client_ip(request.headers()).unwrap_or("-").to_string();
    let user_agent = user_agent(request.headers()).unwrap_or("-").to_string();
    let request_url = request
        .uri()
        .path_and_query()
        .map_or_else(|| path.clone(), |url| url.as_str().to_string());

    let started = Instant::now();
    let response = next.run(request).await;
    let status = response.status().as_u16();
    let mut event = new_event(action(&method, &path), status);
    event.context.insert("ip".to_string(), ip.into());
    event
        .context
        .insert("user_agent".to_string(), user_agent.into());
    event
        .context
        .insert("request_url".to_string(), request_url.into());
    event.latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    logger.record(event);
    response
}

fn action(method: &str, path: &str) -> String {
    if method.is_empty() {
        return "-".to_string();
    }
    let path = if path.is_empty() { "/" } else { path };
    format!("API/{method}:{path}")
}

fn request_path(path: &str) -> String {
    if path.is_empty() {
        "-".to_string()
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
