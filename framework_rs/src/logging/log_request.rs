use std::net::SocketAddr;
use std::time::Instant;

use axum::body::{to_bytes, Body};
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::{Map, Value};

use super::{event, LogLevel, LogShipper};

const BODY_LOG_LIMIT: usize = 64 * 1024;

pub async fn log_request(
    State(shipper): State<LogShipper>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().as_str().to_string();
    let path = request_path(request.uri().path());
    let ip = client_ip(&request);
    let user_agent = user_agent(request.headers());
    let query = request.uri().query().map(query_fields);
    let started = Instant::now();
    let (request, body) = split_json_body(request).await;
    let response = match request {
        Some(request) => next.run(request).await,
        None => StatusCode::BAD_REQUEST.into_response(),
    };
    let status = response.status().as_u16();
    let mut event = event(action(&method, &path), "request", level_for_status(status));
    event.ip = ip;
    event.user_agent = user_agent;
    event.status = Some(status);
    event.latency_ms = Some(millis_since(started));
    for (key, fields) in [("query", query), ("body", body)] {
        if let Some(fields) = fields.filter(|fields| !fields.is_empty()) {
            event.extra.insert(key.to_string(), Value::Object(fields));
        }
    }
    shipper.record(event);
    response
}

async fn split_json_body(request: Request) -> (Option<Request>, Option<Map<String, Value>>) {
    if !is_small_json_body(request.headers()) {
        return (Some(request), None);
    }
    let (parts, body) = request.into_parts();
    let Ok(bytes) = to_bytes(body, BODY_LOG_LIMIT).await else {
        return (None, None);
    };
    let fields = serde_json::from_slice::<Map<String, Value>>(&bytes).ok();
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

fn query_fields(query: &str) -> Map<String, Value> {
    let mut fields = Map::new();
    for (key, value) in form_urlencoded::parse(query.as_bytes()) {
        let value = Value::String(value.into_owned());
        match fields.get_mut(key.as_ref()) {
            None => {
                fields.insert(key.into_owned(), value);
            }
            Some(Value::Array(values)) => values.push(value),
            Some(first) => *first = Value::Array(vec![first.take(), value]),
        }
    }
    fields
}

fn action(method: &str, path: &str) -> String {
    if method.is_empty() {
        return String::new();
    }
    let path = if path.is_empty() { "/" } else { path };
    format!("{method} {path}")
}

fn request_path(path: &str) -> String {
    if path.is_empty() {
        "/".to_string()
    } else {
        path.to_string()
    }
}

pub(super) fn client_ip(request: &Request) -> String {
    if let Some(ip) = first_forwarded_ip(request.headers()) {
        return ip;
    }
    if let Some(ip) = header_text(request.headers(), "x-real-ip") {
        return ip;
    }
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip().to_string())
        .unwrap_or_default()
}

fn first_forwarded_ip(headers: &HeaderMap) -> Option<String> {
    let value = header_text(headers, "x-forwarded-for")?;
    let first = value.split(',').next()?.trim();
    if first.is_empty() {
        None
    } else {
        Some(first.to_string())
    }
}

pub(super) fn user_agent(headers: &HeaderMap) -> String {
    header_text(headers, "user-agent").unwrap_or_default()
}

fn header_text(headers: &HeaderMap, name: &str) -> Option<String> {
    let value = headers.get(name)?.to_str().ok()?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn level_for_status(status: u16) -> LogLevel {
    if status >= 500 {
        LogLevel::Error
    } else if status >= 400 {
        LogLevel::Warn
    } else {
        LogLevel::Info
    }
}

fn millis_since(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::{LogShipper, ShipperConfig};
    use axum::body::Body;
    use axum::extract::Path;
    use axum::http::{Request, StatusCode};
    use axum::middleware::from_fn_with_state;
    use axum::routing::{get, post};
    use axum::Router;
    use serde_json::json;
    use tower::ServiceExt;

    async fn status_from_path(Path(code): Path<u16>) -> StatusCode {
        StatusCode::from_u16(code).unwrap_or(StatusCode::BAD_REQUEST)
    }

    fn shipper() -> LogShipper {
        let mut config = ShipperConfig::new("api");
        config.endpoint = Some("http://logs.example".to_string());
        LogShipper::new(config).unwrap()
    }

    async fn echo(body: String) -> String {
        body
    }

    fn app(shipper: LogShipper) -> Router {
        Router::new()
            .route("/status/{code}", get(status_from_path))
            .route("/echo", post(echo))
            .layer(from_fn_with_state(shipper, log_request))
    }

    async fn post_body(
        shipper: &LogShipper,
        uri: &str,
        content_type: &str,
        body: String,
    ) -> (StatusCode, String) {
        let response = app(shipper.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("content-type", content_type)
                    .header("content-length", body.len())
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    async fn send(app: Router, uri: &str) -> axum::response::Response {
        app.oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    #[test]
    fn action_joins_method_and_path() {
        assert_eq!(action("GET", "/api/foo"), "GET /api/foo");
        assert_eq!(action("POST", ""), "POST /");
        assert_eq!(action("", "/api/foo"), "");
        assert_eq!(action("", ""), "");
    }

    #[tokio::test]
    async fn middleware_records_the_request_schema() {
        let shipper = shipper();
        let response = app(shipper.clone())
            .oneshot(
                Request::builder()
                    .uri("/status/204?token=secret")
                    .header("x-request-id", "req-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(response.headers().get("x-request-id").is_none());
        let events = shipper.buffered_events();
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.id, "");
        assert_eq!(event.action, "GET /status/204");
        assert_eq!(event.ip, "");
        assert_eq!(event.user_agent, "");
        assert_eq!(event.status, Some(204));
        assert!(event.latency_ms.is_some());
        assert_eq!(event.level, LogLevel::Info);
        assert_eq!(event.message, "request");
        let json = serde_json::to_string(event).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value.get("app").is_none());
        assert!(value.get("project").is_none());
        assert_eq!(value["id"], "");
        assert_eq!(value["action"], "GET /status/204");
        assert!(!json.contains("req-1"));
        assert!(value.get("request").is_none());
        assert_eq!(value["extra"], json!({"query": {"token": "***"}}));
    }

    #[tokio::test]
    async fn middleware_leaves_out_empty_fields() {
        let shipper = shipper();
        let body = json!({"name": "ian"}).to_string();
        post_body(&shipper, "/echo?", "application/json", body).await;
        let event = &shipper.buffered_events()[0];
        assert_eq!(event.extra.len(), 1);
        assert_eq!(event.extra["body"], json!({"name": "ian"}));
        assert!(!event.extra.contains_key("query"));
    }

    #[tokio::test]
    async fn middleware_logs_each_query_field() {
        let shipper = shipper();
        send(app(shipper.clone()), "/status/200?a=1&b=x%20y&a=2&flag").await;
        let event = &shipper.buffered_events()[0];
        assert_eq!(
            event.extra["query"],
            json!({"a": ["1", "2"], "b": "x y", "flag": ""})
        );
    }

    #[tokio::test]
    async fn middleware_logs_each_json_body_field_and_keeps_the_body() {
        let shipper = shipper();
        let body = json!({"name": "ian", "age": 3, "tags": ["a"]}).to_string();
        let (status, echoed) = post_body(
            &shipper,
            "/echo?x=1",
            "application/json; charset=utf-8",
            body.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(echoed, body);
        let event = &shipper.buffered_events()[0];
        assert_eq!(event.action, "POST /echo");
        assert_eq!(
            event.extra["body"],
            json!({"name": "ian", "age": 3, "tags": ["a"]})
        );
        assert_eq!(event.extra["query"], json!({"x": "1"}));
    }

    #[tokio::test]
    async fn middleware_skips_a_body_it_cannot_log() {
        let shipper = shipper();
        let large = json!({"data": "x".repeat(BODY_LOG_LIMIT)}).to_string();
        let cases = [
            ("text/plain", "name=ian".to_string()),
            ("application/x-www-form-urlencoded", "name=ian".to_string()),
            ("application/json", "[1, 2]".to_string()),
            ("application/json", "{not json".to_string()),
            ("application/json", large),
        ];
        for (content_type, body) in cases {
            let (status, echoed) = post_body(&shipper, "/echo", content_type, body.clone()).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(echoed, body);
        }
        for event in shipper.buffered_events() {
            assert!(!event.extra.contains_key("body"));
        }
    }

    #[tokio::test]
    async fn middleware_masks_query_and_body_fields_by_key() {
        let mut config = ShipperConfig::new("api");
        config.endpoint = Some("http://logs.example".to_string());
        config.mask_patterns = vec![r"^(password|token)$".to_string()];
        let shipper = LogShipper::new(config).unwrap();
        let body = json!({"user": "ian", "password": "hunter2"}).to_string();
        post_body(&shipper, "/echo?token=abc&page=2", "application/json", body).await;
        let event = &shipper.buffered_events()[0];
        assert_eq!(event.extra["query"], json!({"token": "***", "page": "2"}));
        assert_eq!(
            event.extra["body"],
            json!({"user": "ian", "password": "***"})
        );
    }

    #[tokio::test]
    async fn middleware_levels_follow_the_status() {
        let shipper = shipper();
        let router = app(shipper.clone());
        assert_eq!(
            send(router.clone(), "/status/404").await.status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            send(router, "/status/503").await.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        let events = shipper.buffered_events();
        assert_eq!(events[0].level, LogLevel::Warn);
        assert_eq!(events[0].status, Some(404));
        assert_eq!(events[1].level, LogLevel::Error);
        assert_eq!(events[1].status, Some(503));
    }

    #[tokio::test]
    async fn middleware_reads_ip_and_user_agent() {
        let shipper = shipper();
        let response = app(shipper.clone())
            .oneshot(
                Request::builder()
                    .uri("/status/200")
                    .header("x-forwarded-for", " 203.0.113.8, 10.0.0.1 ")
                    .header("x-real-ip", "198.51.100.4")
                    .header("user-agent", " demo-agent ")
                    .extension(ConnectInfo(SocketAddr::from(([192, 0, 2, 10], 443))))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let event = &shipper.buffered_events()[0];
        assert_eq!(event.ip, "203.0.113.8");
        assert_eq!(event.user_agent, "demo-agent");
        assert_eq!(event.action, "GET /status/200");
    }

    #[tokio::test]
    async fn middleware_uses_real_ip_then_the_socket_host() {
        let shipper = shipper();
        let router = app(shipper.clone());
        router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/status/200")
                    .header("x-real-ip", "198.51.100.4")
                    .extension(ConnectInfo(SocketAddr::from(([192, 0, 2, 10], 443))))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        router
            .oneshot(
                Request::builder()
                    .uri("/status/201")
                    .extension(ConnectInfo(SocketAddr::from(([192, 0, 2, 10], 443))))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let events = shipper.buffered_events();
        assert_eq!(events[0].ip, "198.51.100.4");
        assert_eq!(events[0].user_agent, "");
        assert_eq!(events[1].ip, "192.0.2.10");
        assert_eq!(events[1].action, "GET /status/201");
    }
}
