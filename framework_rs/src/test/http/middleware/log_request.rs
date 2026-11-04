use axum::body::{to_bytes, Body};
use axum::extract::Request;
use axum::http::StatusCode;

use crate::logging::LogShipper;

use crate::http::middleware::log_request::*;
use crate::logging::LogLevel;
use crate::test::logging::buffered_events;
use crate::test::logging::shipper_config;
use axum::extract::{ConnectInfo, Path};
use axum::middleware::from_fn_with_state;
use axum::routing::{get, post};
use axum::Router;
use serde_json::json;
use std::net::SocketAddr;

use tower::ServiceExt;

async fn status_from_path(Path(code): Path<u16>) -> StatusCode {
    StatusCode::from_u16(code).unwrap_or(StatusCode::BAD_REQUEST)
}

fn shipper() -> LogShipper {
    let config = shipper_config("api");
    LogShipper::new(config)
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
    let events = buffered_events(&shipper);
    assert_eq!(events.len(), 1);
    let event = &events[0];
    crate::test::logging::assert_uuid_v7(&event.id);
    assert_eq!(event.action, "API/GET:/status/204");
    assert_eq!(event.context["ip"], "");
    assert_eq!(event.context["user_agent"], "");
    assert_eq!(event.context["request_url"], "/status/204?***");
    assert!(event.latency_ms.is_some());
    assert_eq!(event.level, LogLevel::Info);
    let json = serde_json::to_string(event).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(value.get("app").is_none());
    assert!(value.get("project").is_none());
    crate::test::logging::assert_uuid_v7(value["id"].as_str().unwrap());
    assert_eq!(value["action"], "API/GET:/status/204");
    assert!(!json.contains("req-1"));
    assert!(value.get("request").is_none());
    assert_eq!(value["extra"], json!({}));
    assert_eq!(value["context"]["request_url"], "/status/204?***");
}

#[tokio::test]
async fn middleware_leaves_out_empty_fields() {
    let shipper = shipper();
    let body = json!({"name": "alice"}).to_string();
    post_body(&shipper, "/echo?", "application/json", body).await;
    let event = &buffered_events(&shipper)[0];
    assert_eq!(event.extra.len(), 1);
    assert_eq!(event.extra["name"], "alice");
}

#[tokio::test]
async fn middleware_logs_the_request_url_with_its_query() {
    let shipper = shipper();
    send(app(shipper.clone()), "/status/200?a=1&b=x%20y&flag").await;
    let event = &buffered_events(&shipper)[0];
    assert_eq!(event.context["request_url"], "/status/200?a=1&b=x%20y&flag");
    assert!(!event.extra.contains_key("query"));
}

#[tokio::test]
async fn middleware_logs_each_json_body_field_and_keeps_the_body() {
    let shipper = shipper();
    let body = json!({"name": "alice", "age": 3, "ok": true}).to_string();
    let (status, echoed) = post_body(
        &shipper,
        "/echo?x=1",
        "application/json; charset=utf-8",
        body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(echoed, body);
    let event = &buffered_events(&shipper)[0];
    assert_eq!(event.action, "API/POST:/echo");
    assert_eq!(event.extra["name"], "alice");
    assert_eq!(event.extra["age"], json!(3));
    assert_eq!(event.extra["ok"], json!(true));
    assert_eq!(event.context["request_url"], "/echo?x=1");
}

#[tokio::test]
async fn middleware_skips_a_body_it_cannot_log() {
    let shipper = shipper();
    let large = json!({"data": "x".repeat(BODY_LOG_LIMIT)}).to_string();
    let cases = [
        ("text/plain", "name=alice".to_string()),
        (
            "application/x-www-form-urlencoded",
            "name=alice".to_string(),
        ),
        ("application/json", "[1, 2]".to_string()),
        ("application/json", r#"{"nested": {"a": 1}}"#.to_string()),
        ("application/json", r#"{"list": [1]}"#.to_string()),
        ("application/json", r#"{"none": null}"#.to_string()),
        ("application/json", "{not json".to_string()),
        ("application/json", large),
    ];
    for (content_type, body) in cases {
        let (status, echoed) = post_body(&shipper, "/echo", content_type, body.clone()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(echoed, body);
    }
    for event in buffered_events(&shipper) {
        assert!(event.extra.is_empty());
    }
}

#[tokio::test]
async fn middleware_masks_request_url_and_body_fields() {
    let mut config = shipper_config("api");
    config.mask_patterns = vec![r"password".to_string(), r"token=[^&]+".to_string()];
    let shipper = LogShipper::new(config);
    let body = json!({"user": "alice", "password": "hunter2"}).to_string();
    post_body(&shipper, "/echo?token=abc&page=2", "application/json", body).await;
    let event = &buffered_events(&shipper)[0];
    assert_eq!(event.context["request_url"], "/echo?***&page=2");
    assert_eq!(event.extra["user"], "alice");
    assert_eq!(event.extra["password"], "***");
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
    let events = buffered_events(&shipper);
    assert_eq!(events[0].level, LogLevel::Warn);
    assert_eq!(events[1].level, LogLevel::Error);
}

#[tokio::test]
async fn middleware_reads_ip_and_user_agent() {
    let shipper = shipper();
    let response = app(shipper.clone())
        .oneshot(
            Request::builder()
                .uri("/status/200")
                .header("cf-connecting-ip", " 203.0.113.9 ")
                .header("x-forwarded-for", "203.0.113.8, 10.0.0.1")
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
    assert_eq!(event.context["ip"], "203.0.113.9");
    assert_eq!(event.context["user_agent"], "demo-agent");
    assert_eq!(event.action, "API/GET:/status/200");
}

#[tokio::test]
async fn middleware_ignores_every_source_but_cloudflare() {
    let shipper = shipper();
    app(shipper.clone())
        .oneshot(
            Request::builder()
                .uri("/status/200")
                .header("x-forwarded-for", "203.0.113.8, 10.0.0.1")
                .header("x-real-ip", "198.51.100.4")
                .extension(ConnectInfo(SocketAddr::from(([192, 0, 2, 10], 443))))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let events = buffered_events(&shipper);
    assert_eq!(events[0].context["ip"], "");
    assert_eq!(events[0].context["user_agent"], "");
}
