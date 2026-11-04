use axum::body::to_bytes;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::from_fn;
use axum::routing::post;
use axum::Router;

use crate::http::middleware::{client_ip, require_same_origin, user_agent};

use super::{LogEvent, LogShipper, SHIP_CHUNK};

const CLIENT_LOG_PATH: &str = "/log";
const BODY_LIMIT: usize = 1024 * 1024;

pub fn client_events(events: &[LogEvent]) -> Option<Vec<LogEvent>> {
    if events.is_empty() || events.len() > SHIP_CHUNK {
        return None;
    }
    Some(events.iter().map(LogEvent::from_client).collect())
}

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
    let Ok(events) = serde_json::from_slice::<Vec<LogEvent>>(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    let Some(events) = client_events(&events) else {
        return StatusCode::BAD_REQUEST;
    };
    for mut event in events {
        event.context.insert("ip".to_string(), ip.clone());
        event
            .context
            .insert("user_agent".to_string(), user_agent.clone());
        shipper.record(event);
    }
    StatusCode::NO_CONTENT
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::middleware::log_request;
    use crate::logging::tests::buffered_events;
    use crate::logging::{LogEvent, LogLevel, ShipperConfig, SHIP_CHUNK};
    use axum::body::Body;
    use axum::middleware::from_fn_with_state;
    use serde_json::json;
    use tower::ServiceExt;

    fn shipper() -> LogShipper {
        let mut config = ShipperConfig::new("web");
        config.endpoint = Some("https://logs.example".to_string());
        LogShipper::new(config).unwrap()
    }

    fn client_event(action: &str) -> serde_json::Value {
        json!({
            "ts": "2026-10-25T12:00:00Z",
            "id": "client-id",
            "action": action,
            "latency_ms": 5,
            "level": "error",
            "error_message": "client failed",
            "context": { "ip": "10.0.0.1", "user_agent": "fake-agent" },
            "info": { "page": "/home" },
        })
    }

    fn batch_body(actions: &[&str]) -> Body {
        let events: Vec<_> = actions.iter().map(|action| client_event(action)).collect();
        Body::from(json!(events).to_string())
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
            ("cf-connecting-ip", "203.0.113.8"),
            ("user-agent", "demo-agent"),
        ];
        let status = send(&shipper, &headers, batch_body(&["boom", "again"])).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let events = buffered_events(&shipper);
        assert_eq!(events.len(), 2);
        let event = &events[0];
        assert_eq!(event.action, "boom");
        assert_eq!(event.level, LogLevel::Error);
        assert_eq!(event.context["ip"], "203.0.113.8");
        assert_eq!(event.context["user_agent"], "demo-agent");
        assert_eq!(event.id, "");
        assert_eq!(event.latency_ms, None);
        assert_eq!(event.info["page"], "/home");
    }

    #[tokio::test]
    async fn an_error_event_without_a_message_does_not_reject_the_batch() {
        let shipper = shipper();
        let mut bad = client_event("old");
        bad.as_object_mut().unwrap().remove("error_message");
        let body = json!([bad, client_event("good")]).to_string();
        let headers = [("sec-fetch-site", "same-origin")];
        let status = send(&shipper, &headers, Body::from(body)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let events = buffered_events(&shipper);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].error_message(), Some("no error message"));
        assert_eq!(events[1].error_message(), Some("client failed"));
    }

    #[tokio::test]
    async fn a_non_string_error_message_does_not_reject_the_batch() {
        let shipper = shipper();
        let mut number = client_event("number");
        number["error_message"] = json!(123);
        let mut object = client_event("object");
        object["error_message"] = json!({"a": 1});
        let body = json!([number, object, client_event("good")]).to_string();
        let headers = [("sec-fetch-site", "same-origin")];
        let status = send(&shipper, &headers, Body::from(body)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let events = buffered_events(&shipper);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].error_message(), Some("no error message"));
        assert_eq!(events[1].error_message(), Some("no error message"));
        assert_eq!(events[2].error_message(), Some("client failed"));
    }

    #[tokio::test]
    async fn ignores_every_ip_source_but_cloudflare() {
        let shipper = shipper();
        let headers = [
            ("sec-fetch-site", "same-origin"),
            ("x-forwarded-for", "203.0.113.8"),
            ("x-real-ip", "203.0.113.9"),
        ];
        let status = send(&shipper, &headers, batch_body(&["boom"])).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(buffered_events(&shipper)[0].context["ip"], "");
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
        assert!(buffered_events(&shipper).is_empty());
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
        assert_eq!(buffered_events(&shipper).len(), 1);
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
            send(
                &shipper,
                &headers,
                Body::from(json!({ "events": [client_event("boom")] }).to_string()),
            )
            .await,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            send(&shipper, &headers, Body::from(vec![b' '; BODY_LIMIT + 1])).await,
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert!(buffered_events(&shipper).is_empty());
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
        let events = buffered_events(&shipper);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].action, "boom");
        assert_eq!(events[1].action, "API/POST:/log");
    }

    #[test]
    fn client_events_rejects_an_empty_list() {
        assert!(client_events(&[]).is_none());
    }

    #[test]
    fn client_events_returns_sanitized_events() {
        let stored =
            client_events(&[LogEvent::info("/a?x=1"), LogEvent::error("#frag", "boom")]).unwrap();
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].action, "/a?x=1");
        assert_eq!(stored[0].level, LogLevel::Info);
        assert_eq!(stored[0].id, "");
        assert!(stored[0].context.is_empty());
        assert_eq!(stored[0].latency_ms, None);
        assert_eq!(stored[1].action, "#frag");
        assert_eq!(stored[1].level, LogLevel::Error);
    }

    #[test]
    fn client_events_accepts_a_list_at_the_ship_chunk() {
        let events: Vec<_> = (0..SHIP_CHUNK).map(|_| LogEvent::info("/ok")).collect();
        let stored = client_events(&events).unwrap();
        assert_eq!(stored.len(), SHIP_CHUNK);
        assert_eq!(stored[0].id, "");
    }

    #[test]
    fn client_events_rejects_a_list_above_the_ship_chunk() {
        let events: Vec<_> = (0..=SHIP_CHUNK).map(|_| LogEvent::info("/ok")).collect();
        assert_eq!(events.len(), SHIP_CHUNK + 1);
        assert!(client_events(&events).is_none());
    }
}
