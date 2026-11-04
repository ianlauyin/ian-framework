use axum::extract::Request;
use axum::http::StatusCode;
use axum::Router;

use crate::logging::log_shipper::SHIP_CHUNK;
use crate::logging::{LogEvent, LogShipper};

use crate::http::middleware::log_request;
use crate::logging::client_log::*;
use crate::logging::LogLevel;
use crate::test::logging::{buffered_events, shipper_config};
use axum::body::Body;
use axum::middleware::from_fn_with_state;
use serde_json::json;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tower::ServiceExt;

fn shipper() -> LogShipper {
    let mut config = shipper_config("web");
    config.endpoint = "http://127.0.0.1:9".to_string();
    LogShipper::new(config)
}

async fn shipper_with_server(status: StatusCode) -> (LogShipper, UnboundedReceiver<Vec<LogEvent>>) {
    let (tx, rx) = unbounded_channel();
    let app = Router::new().route(
        "/log/web",
        axum::routing::post(move |body: String| async move {
            tx.send(serde_json::from_str(&body).unwrap()).unwrap();
            status
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut config = shipper_config("web");
    config.endpoint = format!("http://{addr}");
    (LogShipper::new(config), rx)
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
        "extra": { "page": "/home" },
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
async fn sends_a_same_origin_batch_with_the_request_ip_and_user_agent() {
    let (shipper, mut rx) = shipper_with_server(StatusCode::NO_CONTENT).await;
    let headers = [
        ("sec-fetch-site", "same-origin"),
        ("cf-connecting-ip", "203.0.113.8"),
        ("user-agent", "demo-agent"),
    ];
    let status = send(&shipper, &headers, batch_body(&["boom", "again"])).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let events = rx.try_recv().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(shipper.buffered_len(), 0);
    let event = &events[0];
    assert_eq!(event.action, "boom");
    assert_eq!(event.level, LogLevel::Error);
    assert_eq!(event.context["ip"], "203.0.113.8");
    assert_eq!(event.context["user_agent"], "demo-agent");
    assert_eq!(event.id, "");
    assert_eq!(event.latency_ms, None);
    assert_eq!(event.extra["page"], "/home");
}

#[tokio::test]
async fn an_error_event_without_a_message_does_not_reject_the_batch() {
    let (shipper, mut rx) = shipper_with_server(StatusCode::NO_CONTENT).await;
    let mut bad = client_event("old");
    bad.as_object_mut().unwrap().remove("error_message");
    let body = json!([bad, client_event("good")]).to_string();
    let headers = [("sec-fetch-site", "same-origin")];
    let status = send(&shipper, &headers, Body::from(body)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let events = rx.try_recv().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].error_message(), Some("no error message"));
    assert_eq!(events[1].error_message(), Some("client failed"));
}

#[tokio::test]
async fn a_non_string_error_message_rejects_the_batch() {
    let shipper = shipper();
    let mut number = client_event("number");
    number["error_message"] = json!(123);
    let body = json!([number, client_event("good")]).to_string();
    let headers = [("sec-fetch-site", "same-origin")];
    let status = send(&shipper, &headers, Body::from(body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(buffered_events(&shipper).is_empty());
}

#[tokio::test]
async fn ignores_every_ip_source_but_cloudflare() {
    let (shipper, mut rx) = shipper_with_server(StatusCode::NO_CONTENT).await;
    let headers = [
        ("sec-fetch-site", "same-origin"),
        ("x-forwarded-for", "203.0.113.8"),
        ("x-real-ip", "203.0.113.9"),
    ];
    let status = send(&shipper, &headers, batch_body(&["boom"])).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(rx.try_recv().unwrap()[0].context["ip"], "");
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
    let (shipper, mut rx) = shipper_with_server(StatusCode::NO_CONTENT).await;
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
    assert_eq!(rx.try_recv().unwrap().len(), 1);
    assert!(rx.try_recv().is_err());
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
async fn gives_bad_gateway_when_the_log_server_refuses_the_batch() {
    let (shipper, _rx) = shipper_with_server(StatusCode::SERVICE_UNAVAILABLE).await;
    let headers = [("sec-fetch-site", "same-origin")];
    let status = send(&shipper, &headers, batch_body(&["boom"])).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(shipper.buffered_len(), 0);
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
async fn sends_client_events_and_buffers_the_request_event() {
    let (shipper, mut rx) = shipper_with_server(StatusCode::NO_CONTENT).await;
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
    let sent = rx.try_recv().unwrap();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].action, "boom");
    let events = buffered_events(&shipper);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].action, "API/POST:/log");
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
