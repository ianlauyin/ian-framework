use axum::body::to_bytes;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::from_fn;
use axum::routing::post;
use axum::Router;

use crate::http::require_same_origin;

use super::log_request::{client_ip, user_agent};
use super::{LogBatch, LogShipper};

const CLIENT_LOG_PATH: &str = "/log";
const BODY_LIMIT: usize = 1024 * 1024;

pub fn client_log_router(shipper: LogShipper) -> Router {
    Router::new()
        .route(
            CLIENT_LOG_PATH,
            post(receive).route_layer(from_fn(require_same_origin)),
        )
        .with_state(shipper)
}

async fn receive(State(shipper): State<LogShipper>, request: Request) -> StatusCode {
    let ip = client_ip(&request);
    let user_agent = user_agent(request.headers());
    let Ok(body) = to_bytes(request.into_body(), BODY_LIMIT).await else {
        return StatusCode::PAYLOAD_TOO_LARGE;
    };
    let Ok(batch) = serde_json::from_slice::<LogBatch>(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    let Some(events) = batch.sanitize_untrusted_client() else {
        return StatusCode::BAD_REQUEST;
    };
    for mut event in events {
        event.ip = ip.clone();
        event.user_agent = user_agent.clone();
        shipper.record(event);
    }
    StatusCode::NO_CONTENT
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::{log_request, LogLevel, ShipperConfig};
    use axum::body::Body;
    use axum::middleware::from_fn_with_state;
    use serde_json::json;
    use tower::ServiceExt;

    fn shipper() -> LogShipper {
        let mut config = ShipperConfig::new("web");
        config.endpoint = Some("http://logs.example".to_string());
        LogShipper::new(config).unwrap()
    }

    fn client_event(action: &str) -> serde_json::Value {
        json!({
            "ts": "2026-10-25T12:00:00Z",
            "id": "client-id",
            "ip": "10.0.0.1",
            "user_agent": "fake-agent",
            "action": action,
            "latency_ms": 5,
            "level": "error",
            "extra": { "page": "/home" },
        })
    }

    fn batch_body(actions: &[&str]) -> Body {
        let events: Vec<_> = actions.iter().map(|action| client_event(action)).collect();
        Body::from(json!({ "events": events }).to_string())
    }

    async fn send(shipper: &LogShipper, headers: &[(&str, &str)], body: Body) -> StatusCode {
        let mut request = Request::builder().method("POST").uri("/log");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        client_log_router(shipper.clone())
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn records_a_same_origin_batch_with_the_request_ip_and_user_agent() {
        let shipper = shipper();
        let headers = [
            ("sec-fetch-site", "same-origin"),
            ("x-real-ip", "203.0.113.8"),
            ("user-agent", "demo-agent"),
        ];
        let status = send(&shipper, &headers, batch_body(&["boom", "again"])).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let events = shipper.buffered_events();
        assert_eq!(events.len(), 2);
        let event = &events[0];
        assert_eq!(event.action, "boom");
        assert_eq!(event.level, LogLevel::Error);
        assert_eq!(event.ip, "203.0.113.8");
        assert_eq!(event.user_agent, "demo-agent");
        assert_eq!(event.id, "");
        assert_eq!(event.latency_ms, None);
        assert_eq!(event.extra["page"], "/home");
    }

    #[tokio::test]
    async fn rejects_a_request_from_another_site() {
        let shipper = shipper();
        for site in ["same-site", "cross-site", "none"] {
            let headers = [("sec-fetch-site", site)];
            let status = send(&shipper, &headers, batch_body(&["boom"])).await;
            assert_eq!(status, StatusCode::FORBIDDEN);
        }
        let headers = [
            ("sec-fetch-site", "cross-site"),
            ("origin", "https://app.example"),
            ("host", "app.example"),
        ];
        let status = send(&shipper, &headers, batch_body(&["boom"])).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(shipper.buffered_events().is_empty());
    }

    #[tokio::test]
    async fn falls_back_to_origin_and_host() {
        let shipper = shipper();
        let same = [
            ("origin", "https://App.example:8443"),
            ("host", "app.example:8443"),
        ];
        assert_eq!(
            send(&shipper, &same, batch_body(&["boom"])).await,
            StatusCode::NO_CONTENT
        );
        let other = [("origin", "https://evil.example"), ("host", "app.example")];
        assert_eq!(
            send(&shipper, &other, batch_body(&["boom"])).await,
            StatusCode::FORBIDDEN
        );
        let null = [("origin", "null"), ("host", "app.example")];
        assert_eq!(
            send(&shipper, &null, batch_body(&["boom"])).await,
            StatusCode::FORBIDDEN
        );
        let no_host = [("origin", "https://app.example")];
        assert_eq!(
            send(&shipper, &no_host, batch_body(&["boom"])).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            send(&shipper, &[], batch_body(&["boom"])).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(shipper.buffered_events().len(), 1);
    }

    #[tokio::test]
    async fn rejects_a_bad_body() {
        let shipper = shipper();
        let headers = [("sec-fetch-site", "same-origin")];
        assert_eq!(
            send(&shipper, &headers, Body::from("not json")).await,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            send(&shipper, &headers, batch_body(&[])).await,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            send(&shipper, &headers, Body::from(vec![b' '; BODY_LIMIT + 1])).await,
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert!(shipper.buffered_events().is_empty());
    }

    #[tokio::test]
    async fn other_methods_and_paths_are_not_accepted() {
        let router = client_log_router(shipper());
        let get = Request::builder().uri("/log").body(Body::empty()).unwrap();
        let response = router.clone().oneshot(get).await.unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        let other = Request::builder()
            .method("POST")
            .uri("/other")
            .body(Body::empty())
            .unwrap();
        let response = router.oneshot(other).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn shares_the_queue_with_the_request_middleware() {
        let shipper = shipper();
        let router = Router::new()
            .merge(client_log_router(shipper.clone()))
            .layer(from_fn_with_state(shipper.clone(), log_request));
        let request = Request::builder()
            .method("POST")
            .uri("/log")
            .header("sec-fetch-site", "same-origin")
            .body(batch_body(&["boom"]))
            .unwrap();
        let response = router.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let events = shipper.buffered_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].action, "boom");
        assert_eq!(events[1].action, "POST /log");
    }
}
