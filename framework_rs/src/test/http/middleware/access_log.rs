use axum::body::{to_bytes, Body};
use axum::extract::Request;
use axum::http::StatusCode;

use crate::logging::Logger;

use crate::http::middleware::access_log::*;
use crate::logging::LogLevel;
use crate::test::logging::buffered_events;
use crate::test::logging::default_shipper;
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

fn shipper() -> Logger {
    crate::test::logging::logger(default_shipper("api"))
}

async fn echo(body: String) -> String {
    body
}

async fn enrich(log: AccessLog) -> StatusCode {
    log.context("user_id", "u-1");
    log.context("ip", "198.51.100.1");
    log.extra("order_id", "o-1");
    log.extra("count", 2u64);
    StatusCode::CONFLICT
}

fn app(shipper: Logger) -> Router {
    Router::new()
        .route("/status/{code}", get(status_from_path))
        .route("/echo", post(echo))
        .route("/enrich", get(enrich))
        .layer(from_fn_with_state(shipper, access_log))
}

async fn post_body(
    shipper: &Logger,
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
    assert_eq!(event.context["ip"], "-");
    assert_eq!(event.context["user_agent"], "-");
    assert_eq!(event.context["request_url"], "/status/204?***");
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
async fn middleware_logs_the_request_url_with_its_query() {
    let shipper = shipper();
    send(app(shipper.clone()), "/status/200?a=1&b=x%20y&flag").await;
    let event = &buffered_events(&shipper)[0];
    assert_eq!(event.context["request_url"], "/status/200?a=1&b=x%20y&flag");
    assert!(!event.extra.contains_key("query"));
}

#[tokio::test]
async fn middleware_does_not_log_the_body_and_keeps_it() {
    let shipper = shipper();
    let body = json!({"name": "alice", "age": 3}).to_string();
    let (status, echoed) = post_body(&shipper, "/echo?x=1", "application/json", body.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(echoed, body);
    let event = &buffered_events(&shipper)[0];
    assert_eq!(event.action, "API/POST:/echo");
    assert_eq!(event.context["request_url"], "/echo?x=1");
    assert!(event.extra.is_empty());
}

#[tokio::test]
async fn handler_adds_context_and_extra_to_the_event() {
    let shipper = shipper();
    let response = send(app(shipper.clone()), "/enrich").await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let event = &buffered_events(&shipper)[0];
    assert_eq!(event.level, LogLevel::Warn);
    assert_eq!(event.context["user_id"], "u-1");
    assert_eq!(event.context["ip"], "-");
    assert_eq!(event.context["request_url"], "/enrich");
    assert_eq!(event.extra["order_id"], "o-1");
    assert_eq!(event.extra["count"], json!(2));
}

#[tokio::test]
async fn handler_without_the_middleware_still_runs() {
    let router = Router::new().route("/enrich", get(enrich));
    assert_eq!(send(router, "/enrich").await.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn middleware_masks_the_request_url() {
    let shipper = Logger::new(default_shipper("api"), vec![r"token=[^&]+".to_string()]);
    send(app(shipper.clone()), "/status/200?token=abc&page=2").await;
    let event = &buffered_events(&shipper)[0];
    assert_eq!(event.context["request_url"], "/status/200?***&page=2");
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
    assert_eq!(events[0].context["ip"], "-");
    assert_eq!(events[0].context["user_agent"], "-");
}
