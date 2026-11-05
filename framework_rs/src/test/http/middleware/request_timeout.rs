use std::time::Duration;

use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::Response;

use crate::http::middleware::request_timeout::*;
use axum::body::{to_bytes, Body};
use axum::middleware::from_fn_with_state;
use axum::routing::get;
use axum::Router;
use tower::ServiceExt;

async fn send(handler_ms: u64) -> Response {
    let router = Router::new()
        .route(
            "/",
            get(move || async move {
                tokio::time::sleep(Duration::from_millis(handler_ms)).await;
                "done"
            }),
        )
        .layer(from_fn_with_state(
            RequestTimeout(Duration::from_millis(50)),
            request_timeout,
        ));
    let request = Request::builder().uri("/").body(Body::empty()).unwrap();
    router.oneshot(request).await.unwrap()
}

#[tokio::test]
async fn passes_a_fast_handler() {
    let response = send(0).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn answers_408_when_the_handler_is_slow() {
    let response = send(2_000).await;
    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body, r#"{"message":"request timed out"}"#);
}
