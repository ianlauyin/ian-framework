use std::net::SocketAddr;
use std::time::Instant;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use uuid::Uuid;

use super::{event, record, request_line, LogLevel, LogShipper};

pub async fn log_request(
    State(shipper): State<LogShipper>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().as_str().to_string();
    let path = request_path(request.uri().path());
    let id = incoming_request_id(request.headers()).unwrap_or_else(new_request_id);
    let ip = client_ip(&request);
    let user_agent = user_agent(request.headers());
    let started = Instant::now();
    let mut response = next.run(request).await;
    let status = response.status().as_u16();
    if let Ok(value) = HeaderValue::from_str(&id) {
        response.headers_mut().insert("x-request-id", value);
    }
    let mut event = event(level_for_status(status), "request");
    event.id = id;
    event.ip = ip;
    event.user_agent = user_agent;
    event.request = request_line(&method, &path);
    event.status = Some(status);
    event.latency_ms = Some(millis_since(started));
    let _ = record(&shipper, event);
    response
}

fn request_path(path: &str) -> String {
    if path.is_empty() {
        "/".to_string()
    } else {
        path.to_string()
    }
}

fn incoming_request_id(headers: &HeaderMap) -> Option<String> {
    let value = headers.get("x-request-id")?.to_str().ok()?;
    if valid_request_id(value) {
        Some(value.to_string())
    } else {
        None
    }
}

fn valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= 128
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | ':'))
}

fn client_ip(request: &Request) -> String {
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

fn user_agent(headers: &HeaderMap) -> String {
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

fn new_request_id() -> String {
    Uuid::new_v4().to_string()
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
    use crate::logging::{buffered_events, log_shipper, shipper_config};
    use axum::body::Body;
    use axum::extract::Path;
    use axum::http::{Request, StatusCode};
    use axum::middleware::from_fn_with_state;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    async fn status_from_path(Path(code): Path<u16>) -> StatusCode {
        StatusCode::from_u16(code).unwrap_or(StatusCode::BAD_REQUEST)
    }

    fn app(shipper: LogShipper) -> Router {
        Router::new()
            .route("/status/{code}", get(status_from_path))
            .layer(from_fn_with_state(shipper, log_request))
    }

    async fn send(app: Router, uri: &str, request_id: Option<&str>) -> axum::response::Response {
        let mut builder = Request::builder().uri(uri);
        if let Some(request_id) = request_id {
            builder = builder.header("x-request-id", request_id);
        }
        app.oneshot(builder.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn middleware_records_the_request_schema() {
        let mut config = shipper_config("api");
        config.project = "demo".to_string();
        let shipper = log_shipper(config).unwrap();
        let response = send(
            app(shipper.clone()),
            "/status/204?token=secret",
            Some("req-1"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response
                .headers()
                .get("x-request-id")
                .unwrap()
                .to_str()
                .unwrap(),
            "req-1"
        );
        let events = buffered_events(&shipper);
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.id, "req-1");
        assert_eq!(event.request, "GET /status/204");
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
        assert!(!json.contains("secret"));
        assert!(!json.contains("token"));
    }

    #[tokio::test]
    async fn middleware_levels_follow_the_status() {
        let shipper = log_shipper(shipper_config("api")).unwrap();
        let router = app(shipper.clone());
        assert_eq!(
            send(router.clone(), "/status/404", Some("warn-1"))
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            send(router, "/status/503", Some("err-1")).await.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        let events = buffered_events(&shipper);
        assert_eq!(events[0].level, LogLevel::Warn);
        assert_eq!(events[0].status, Some(404));
        assert_eq!(events[1].level, LogLevel::Error);
        assert_eq!(events[1].status, Some(503));
    }

    #[tokio::test]
    async fn invalid_request_id_is_replaced() {
        let shipper = log_shipper(shipper_config("api")).unwrap();
        let response = send(app(shipper.clone()), "/status/200", Some("bad id")).await;
        let header = response
            .headers()
            .get("x-request-id")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert_ne!(header, "bad id");
        assert!(generated_request_id(&header));
        assert_eq!(buffered_events(&shipper)[0].id, header);
    }

    #[tokio::test]
    async fn middleware_reads_ip_and_user_agent() {
        let shipper = log_shipper(shipper_config("api")).unwrap();
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
        let event = &buffered_events(&shipper)[0];
        assert_eq!(event.ip, "203.0.113.8");
        assert_eq!(event.user_agent, "demo-agent");
        assert_eq!(event.request, "GET /status/200");
    }

    #[tokio::test]
    async fn middleware_uses_real_ip_then_the_socket_host() {
        let shipper = log_shipper(shipper_config("api")).unwrap();
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
        let events = buffered_events(&shipper);
        assert_eq!(events[0].ip, "198.51.100.4");
        assert_eq!(events[0].user_agent, "");
        assert_eq!(events[1].ip, "192.0.2.10");
        assert_eq!(events[1].request, "GET /status/201");
    }

    fn generated_request_id(value: &str) -> bool {
        value.len() == 36
            && value.bytes().enumerate().all(|(index, byte)| {
                if matches!(index, 8 | 13 | 18 | 23) {
                    byte == b'-'
                } else {
                    byte.is_ascii_hexdigit()
                }
            })
    }
}
