use axum::extract::Request;
use axum::http::StatusCode;
use axum::Router;

use crate::logging::LogLevel;

use crate::http::middleware::access_log;
use crate::logging::client_log::router::*;
use crate::test::logging::{
    assert_uuid_v7, buffered_events, global_logger, shipped_batches, wait_for, REFUSED_ACTION,
};
use axum::body::Body;
use axum::middleware::from_fn;
use serde_json::json;
use tower::ServiceExt;

// All tests share the global logger, so each test uses event actions that no other test uses.
fn client_event(action: &str) -> serde_json::Value {
    json!({
        "ts": "2026-10-25T12:00:00Z",
        "id": "client-id",
        "action": action,
        "latencyMs": 5,
        "level": "error",
        "errorMessage": "client failed",
        "context": { "ip": "10.0.0.1", "user_agent": "fake-agent" },
        "extra": { "page": "/home" },
    })
}

fn batch_body(actions: &[&str]) -> Body {
    let events: Vec<_> = actions.iter().map(|action| client_event(action)).collect();
    Body::from(json!({ "events": events }).to_string())
}

fn is_buffered(action: &str) -> bool {
    buffered_events(global_logger())
        .iter()
        .any(|event| event.action == action)
}

async fn send(headers: &[(&str, &str)], body: Body) -> StatusCode {
    global_logger();
    let mut request = Request::builder()
        .method("POST")
        .uri("/log")
        .header("content-type", "application/json");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    client_log_router()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn sends_a_same_origin_batch_with_the_request_ip_and_user_agent() {
    let headers = [
        ("sec-fetch-site", "same-origin"),
        ("cf-connecting-ip", "203.0.113.8"),
        ("user-agent", "demo-agent"),
    ];
    let status = send(&headers, batch_body(&["same-origin", "same-origin-again"])).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    wait_for(|| !shipped_batches("same-origin").is_empty()).await;
    let batches = shipped_batches("same-origin");
    assert_eq!(batches.len(), 1);
    let events = &batches[0];
    assert_eq!(events.len(), 2);
    assert!(!is_buffered("same-origin"));
    let event = &events[0];
    assert_eq!(event.action, "same-origin");
    assert_eq!(event.level, LogLevel::Error);
    assert_eq!(event.context["ip"], "203.0.113.8");
    assert_eq!(event.context["user_agent"], "demo-agent");
    assert_uuid_v7(&event.id);
    assert_eq!(event.latency_ms, 5);
    assert_eq!(event.extra["page"], "/home");
}

#[tokio::test]
async fn an_error_event_without_a_message_does_not_reject_the_batch() {
    let mut bad = client_event("no-message");
    bad.as_object_mut().unwrap().remove("errorMessage");
    let body = json!({ "events": [bad, client_event("with-message")] }).to_string();
    let headers = [("sec-fetch-site", "same-origin")];
    let status = send(&headers, Body::from(body)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    wait_for(|| !shipped_batches("no-message").is_empty()).await;
    let events = shipped_batches("no-message").remove(0);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].error_message(), Some("unknown error"));
    assert_eq!(events[1].error_message(), Some("client failed"));
}

#[tokio::test]
async fn a_non_string_error_message_rejects_the_batch() {
    let mut number = client_event("number-message");
    number["errorMessage"] = json!(123);
    let body = json!({ "events": [number, client_event("beside-number")] }).to_string();
    let headers = [("sec-fetch-site", "same-origin")];
    let status = send(&headers, Body::from(body)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(shipped_batches("beside-number").is_empty());
    assert!(!is_buffered("beside-number"));
}

#[tokio::test]
async fn ignores_every_ip_source_but_cloudflare() {
    let headers = [
        ("sec-fetch-site", "same-origin"),
        ("x-forwarded-for", "203.0.113.8"),
        ("x-real-ip", "203.0.113.9"),
    ];
    let status = send(&headers, batch_body(&["ip-sources"])).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    wait_for(|| !shipped_batches("ip-sources").is_empty()).await;
    assert_eq!(shipped_batches("ip-sources")[0][0].context["ip"], "");
}

#[tokio::test]
async fn rejects_a_request_from_another_site() {
    for site in ["same-site", "cross-site", "none"] {
        let headers = [("sec-fetch-site", site)];
        let status = send(&headers, batch_body(&["other-site"])).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    let headers = [
        ("sec-fetch-site", "cross-site"),
        ("origin", "https://app.example"),
        ("host", "app.example"),
    ];
    let status = send(&headers, batch_body(&["other-site"])).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(shipped_batches("other-site").is_empty());
    assert!(!is_buffered("other-site"));
}

#[tokio::test]
async fn falls_back_to_origin_and_host() {
    let same = [
        ("origin", "https://App.example:8443"),
        ("host", "app.example:8443"),
    ];
    assert_eq!(
        send(&same, batch_body(&["origin-host"])).await,
        StatusCode::NO_CONTENT
    );
    let other = [("origin", "https://evil.example"), ("host", "app.example")];
    assert_eq!(
        send(&other, batch_body(&["origin-host"])).await,
        StatusCode::FORBIDDEN
    );
    let null = [("origin", "null"), ("host", "app.example")];
    assert_eq!(
        send(&null, batch_body(&["origin-host"])).await,
        StatusCode::FORBIDDEN
    );
    let no_host = [("origin", "https://app.example")];
    assert_eq!(
        send(&no_host, batch_body(&["origin-host"])).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(&[], batch_body(&["origin-host"])).await,
        StatusCode::FORBIDDEN
    );
    wait_for(|| !shipped_batches("origin-host").is_empty()).await;
    assert_eq!(shipped_batches("origin-host").len(), 1);
}

#[tokio::test]
async fn rejects_a_bad_body() {
    let headers = [("sec-fetch-site", "same-origin")];
    assert_eq!(
        send(&headers, Body::from("not json")).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(&headers, batch_body(&[])).await,
        StatusCode::BAD_REQUEST
    );
    assert!(buffered_events(global_logger())
        .iter()
        .any(|event| event.action == "CLIENT_LOG/rejected"));
    assert_eq!(
        send(
            &headers,
            Body::from(json!([client_event("bare-array")]).to_string()),
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        send(&headers, Body::from(vec![b' '; BODY_LIMIT + 1])).await,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert!(shipped_batches("bare-array").is_empty());
    assert!(!is_buffered("bare-array"));
}

#[tokio::test]
async fn returns_bad_gateway_when_the_log_server_refuses_the_batch() {
    let headers = [("sec-fetch-site", "same-origin")];
    let status = send(&headers, batch_body(&[REFUSED_ACTION])).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(!is_buffered(REFUSED_ACTION));
}

#[tokio::test]
async fn other_methods_and_paths_are_not_accepted() {
    let router = client_log_router();
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
    global_logger();
    let router = Router::new()
        .merge(client_log_router())
        .layer(from_fn(access_log));
    let request = Request::builder()
        .method("POST")
        .uri("/log")
        .header("sec-fetch-site", "same-origin")
        .header("content-type", "application/json")
        .body(batch_body(&["with-access-log"]))
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    wait_for(|| !shipped_batches("with-access-log").is_empty()).await;
    let sent = shipped_batches("with-access-log").remove(0);
    assert_eq!(sent.len(), 1);
    assert!(!is_buffered("with-access-log"));
    let request_events: Vec<_> = buffered_events(global_logger())
        .into_iter()
        .filter(|event| event.action == "API/POST:/log")
        .collect();
    assert_eq!(request_events.len(), 1);
}
