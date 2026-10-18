use std::time::Instant;

use axum::extract::{Request, State};
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use uuid::Uuid;

use super::{event, record, LogLevel, LogShipper, MAX_REQUEST_ID_BYTES};

pub async fn log_request(
    State(shipper): State<LogShipper>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().as_str().to_string();
    let path = request_path(request.uri().path());
    let request_id = incoming_request_id(request.headers()).unwrap_or_else(new_request_id);
    let started = Instant::now();
    let mut response = next.run(request).await;
    let status = response.status().as_u16();
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert("x-request-id", value);
    }
    let mut event = event("", "", level_for_status(status), "request");
    event.request_id = request_id;
    event.method = method;
    event.path = path;
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

fn incoming_request_id(headers: &axum::http::HeaderMap) -> Option<String> {
    let value = headers.get("x-request-id")?.to_str().ok()?;
    if valid_request_id(value) {
        Some(value.to_string())
    } else {
        None
    }
}

fn valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REQUEST_ID_BYTES
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | ':'))
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
        let shipper = log_shipper(shipper_config("demo", "api")).unwrap();
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
        assert_eq!(event.project, "demo");
        assert_eq!(event.service, "api");
        assert_eq!(event.request_id, "req-1");
        assert_eq!(event.method, "GET");
        assert_eq!(event.path, "/status/204");
        assert_eq!(event.status, Some(204));
        assert!(event.latency_ms.is_some());
        assert_eq!(event.level, LogLevel::Info);
        assert_eq!(event.message, "request");
        let line = crate::logging::to_json_line(event).unwrap();
        assert!(!line.contains("secret"));
        assert!(!line.contains("token"));
    }

    #[tokio::test]
    async fn middleware_levels_follow_the_status() {
        let shipper = log_shipper(shipper_config("demo", "api")).unwrap();
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
        let shipper = log_shipper(shipper_config("demo", "api")).unwrap();
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
        assert_eq!(buffered_events(&shipper)[0].request_id, header);
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
