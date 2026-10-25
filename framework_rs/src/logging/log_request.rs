use std::net::SocketAddr;
use std::time::Instant;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::HeaderMap;
use axum::middleware::Next;
use axum::response::Response;

use super::{event, LogLevel, LogShipper};

pub async fn log_request(
    State(shipper): State<LogShipper>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().as_str().to_string();
    let path = request_path(request.uri().path());
    let ip = client_ip(&request);
    let user_agent = user_agent(request.headers());
    let started = Instant::now();
    let response = next.run(request).await;
    let status = response.status().as_u16();
    let mut event = event(action(&method, &path), "request", level_for_status(status));
    event.ip = ip;
    event.user_agent = user_agent;
    event.status = Some(status);
    event.latency_ms = Some(millis_since(started));
    shipper.record(event);
    response
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
        let shipper = LogShipper::new(ShipperConfig::new("api", "")).unwrap();
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
        assert!(!json.contains("secret"));
        assert!(!json.contains("token"));
    }

    #[tokio::test]
    async fn middleware_levels_follow_the_status() {
        let shipper = LogShipper::new(ShipperConfig::new("api", "")).unwrap();
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
        let shipper = LogShipper::new(ShipperConfig::new("api", "")).unwrap();
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
        let shipper = LogShipper::new(ShipperConfig::new("api", "")).unwrap();
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
