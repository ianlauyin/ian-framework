use axum::body::{to_bytes, Body};
use axum::extract::Request;
use axum::http::StatusCode;

use crate::logging::LogEvent;

use crate::exception;
use crate::exception::Exception;
use crate::http::error_code;
use crate::http::middleware::access_log::*;
use crate::logging::LogLevel;
use crate::test::logging::{buffered_events, global_logger};
use axum::extract::{ConnectInfo, Path};
use axum::middleware::from_fn;
use axum::routing::{get, post};
use axum::Router;
use serde_json::json;
use std::net::SocketAddr;

use tower::ServiceExt;

async fn status_from_path(Path(code): Path<u16>) -> StatusCode {
    StatusCode::from_u16(code).unwrap_or(StatusCode::BAD_REQUEST)
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

async fn hidden_error() -> Result<(), Exception> {
    Err(exception!(
        code = "MY_CODE",
        "db down",
        source = exception!(code = "DB_ERROR", "secret detail"),
    ))
}

async fn client_error() -> Result<(), Exception> {
    Err(exception!(code = error_code::NOT_FOUND, "job not found"))
}

fn app() -> Router {
    global_logger();
    Router::new()
        .route("/status/{code}", get(status_from_path))
        .route("/echo", post(echo))
        .route("/enrich", get(enrich))
        .route("/hidden-error", get(hidden_error))
        .route("/client-error", get(client_error))
        .layer(from_fn(access_log))
}

// All tests share the global logger, so each test uses a request URL that no other test uses.
fn recorded(request_url: &str) -> LogEvent {
    let mut events: Vec<_> = buffered_events(global_logger())
        .into_iter()
        .filter(|event| {
            event
                .context
                .get("request_url")
                .is_some_and(|url| *url == request_url)
        })
        .collect();
    assert_eq!(events.len(), 1);
    events.remove(0)
}

async fn body_text(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

async fn post_body(uri: &str, content_type: &str, body: String) -> (StatusCode, String) {
    let response = app()
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
    let response = app()
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
    let event = recorded("/status/204?***");
    crate::test::logging::assert_uuid_v7(&event.id);
    assert_eq!(event.action, "API/GET:/status/204");
    assert_eq!(event.context["ip"], "-");
    assert_eq!(event.context["user_agent"], "-");
    assert_eq!(event.level, LogLevel::Info);
    let json = serde_json::to_string(&event).unwrap();
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
    send(app(), "/status/200?a=1&b=x%20y&flag").await;
    let event = recorded("/status/200?a=1&b=x%20y&flag");
    assert!(!event.extra.contains_key("query"));
}

#[tokio::test]
async fn middleware_does_not_log_the_body_and_keeps_it() {
    let body = json!({"name": "alice", "age": 3}).to_string();
    let (status, echoed) = post_body("/echo?x=1", "application/json", body.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(echoed, body);
    let event = recorded("/echo?x=1");
    assert_eq!(event.action, "API/POST:/echo");
    assert!(event.extra.is_empty());
}

#[tokio::test]
async fn handler_adds_context_and_extra_to_the_event() {
    let response = send(app(), "/enrich").await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let event = recorded("/enrich");
    assert_eq!(event.level, LogLevel::Warn);
    assert_eq!(event.context["user_id"], "u-1");
    assert_eq!(event.context["ip"], "-");
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
    send(app(), "/status/200?token=abc&page=2").await;
    recorded("/status/200?***&page=2");
}

#[tokio::test]
async fn middleware_levels_follow_the_status() {
    let router = app();
    assert_eq!(
        send(router.clone(), "/status/404?case=levels")
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(router, "/status/503?case=levels").await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(recorded("/status/404?case=levels").level, LogLevel::Warn);
    assert_eq!(recorded("/status/503?case=levels").level, LogLevel::Error);
}

#[tokio::test]
async fn a_hidden_exception_logs_its_trace_and_not_to_the_client() {
    let response = send(app(), "/hidden-error").await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        body_text(response).await,
        r#"{"message":"internal server error"}"#
    );
    let event = recorded("/hidden-error");
    assert_eq!(event.level, LogLevel::Error);
    let message = event.error_message.as_deref().unwrap();
    assert!(message.starts_with("db down\n0: ERROR [MY_CODE] db down at "));
    assert!(message.contains("\n1: ERROR [DB_ERROR] secret detail at "));
}

#[tokio::test]
async fn a_client_exception_logs_a_warn_with_its_message() {
    let response = send(app(), "/client-error").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let event = recorded("/client-error");
    assert_eq!(event.level, LogLevel::Warn);
    let message = event.error_message.as_deref().unwrap();
    assert!(message.starts_with("job not found\n0: ERROR [NOT_FOUND] job not found at "));
}

#[tokio::test]
async fn a_status_without_an_exception_keeps_the_status_message() {
    send(app(), "/status/503").await;
    let event = recorded("/status/503");
    assert_eq!(event.error_message.as_deref(), Some("response status 503"));
}

#[tokio::test]
async fn middleware_reads_ip_and_user_agent() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/status/200?case=cloudflare")
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
    let event = recorded("/status/200?case=cloudflare");
    assert_eq!(event.context["ip"], "203.0.113.9");
    assert_eq!(event.context["user_agent"], "demo-agent");
    assert_eq!(event.action, "API/GET:/status/200");
}

#[tokio::test]
async fn middleware_ignores_every_source_but_cloudflare() {
    app()
        .oneshot(
            Request::builder()
                .uri("/status/200?case=other-sources")
                .header("x-forwarded-for", "203.0.113.8, 10.0.0.1")
                .header("x-real-ip", "198.51.100.4")
                .extension(ConnectInfo(SocketAddr::from(([192, 0, 2, 10], 443))))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let event = recorded("/status/200?case=other-sources");
    assert_eq!(event.context["ip"], "-");
    assert_eq!(event.context["user_agent"], "-");
}
