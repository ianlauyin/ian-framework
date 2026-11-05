use axum::extract::Request;
use axum::http::StatusCode;
use axum::Router;

use crate::logging::{LogEvent, LogLevel, LogRequest, Logger};

use crate::http::middleware::access_log;
use crate::logging::client_log::router::*;
use crate::test::logging::{assert_uuid_v7, buffered_events, http_shipper, logger};
use axum::body::Body;
use axum::middleware::from_fn_with_state;
use axum::Json;
use serde_json::json;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tower::ServiceExt;

fn shipper() -> Logger {
    logger(http_shipper("web", "http://127.0.0.1:9"))
}

async fn shipper_with_server(status: StatusCode) -> (Logger, UnboundedReceiver<Vec<LogEvent>>) {
    let (tx, rx) = unbounded_channel();
    let app = Router::new().route(
        "/log/web",
        axum::routing::post(move |Json(log): Json<LogRequest>| async move {
            tx.send(log.events).unwrap();
            status
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (logger(http_shipper("web", &format!("http://{addr}"))), rx)
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
    Body::from(json!({ "events": events }).to_string())
}

async fn send(shipper: &Logger, headers: &[(&str, &str)], body: Body) -> StatusCode {
    let mut request = Request::builder()
        .method("POST")
        .uri("/log")
        .header("content-type", "application/json");
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
    assert_uuid_v7(&event.id);
    assert_eq!(event.latency_ms, Some(5));
    assert_eq!(event.extra["page"], "/home");
}

#[tokio::test]
async fn an_error_event_without_a_message_does_not_reject_the_batch() {
    let (shipper, mut rx) = shipper_with_server(StatusCode::NO_CONTENT).await;
    let mut bad = client_event("old");
    bad.as_object_mut().unwrap().remove("error_message");
    let body = json!({ "events": [bad, client_event("good")] }).to_string();
    let headers = [("sec-fetch-site", "same-origin")];
    let status = send(&shipper, &headers, Body::from(body)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let events = rx.try_recv().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].error_message(), Some("unknown error"));
    assert_eq!(events[1].error_message(), Some("client failed"));
}

#[tokio::test]
async fn a_non_string_error_message_rejects_the_batch() {
    let shipper = shipper();
    let mut number = client_event("number");
    number["error_message"] = json!(123);
    let body = json!({ "events": [number, client_event("good")] }).to_string();
    let headers = [("sec-fetch-site", "same-origin")];
    let status = send(&shipper, &headers, Body::from(body)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
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
            Body::from(json!([client_event("boom")]).to_string()),
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY
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
        .layer(from_fn_with_state(shipper.clone(), access_log));
    let request = Request::builder()
        .method("POST")
        .uri("/log")
        .header("sec-fetch-site", "same-origin")
        .header("content-type", "application/json")
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
