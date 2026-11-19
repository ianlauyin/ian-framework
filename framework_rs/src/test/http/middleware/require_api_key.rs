use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::from_fn_with_state;
use axum::routing::any;
use tower::ServiceExt;

use crate::http::middleware::require_api_key::*;

const KEY: &str = "api-key-value";

fn router() -> Router {
    Router::new()
        .fallback(any(|| async { StatusCode::NO_CONTENT }))
        .layer(from_fn_with_state(ApiKey::new(KEY), require_api_key))
}

fn request(path: &str, key: Option<&str>) -> Request {
    let mut request = Request::builder().uri(path);
    if let Some(key) = key {
        request = request.header("authorization", format!("Bearer {key}"));
    }
    request.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn passes_a_request_with_the_key() {
    let response = router().oneshot(request("/any", Some(KEY))).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn rejects_a_missing_or_wrong_key_with_401() {
    for key in [None, Some("wrong"), Some("")] {
        let response = router().oneshot(request("/any", key)).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
async fn rejects_an_empty_configured_key() {
    let router = Router::new()
        .fallback(any(|| async { StatusCode::NO_CONTENT }))
        .layer(from_fn_with_state(ApiKey::new(""), require_api_key));
    let response = router.oneshot(request("/any", Some(""))).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn the_rejection_body_is_json() {
    let response = router().oneshot(request("/any", None)).await.unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body, r#"{"message":"unauthorized"}"#);
}

#[tokio::test]
async fn calls_on_reject_only_for_a_rejected_request() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let key = ApiKey::new(KEY).on_reject(move |_: Request| {
        let counter = counter.clone();
        async move {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    });
    let router = Router::new()
        .fallback(any(|| async { StatusCode::NO_CONTENT }))
        .layer(from_fn_with_state(key, require_api_key));
    router
        .clone()
        .oneshot(request("/any", Some(KEY)))
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    router.oneshot(request("/any", None)).await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn awaits_on_reject_before_the_unauthorized_response() {
    let done = Arc::new(AtomicBool::new(false));
    let flag = done.clone();
    let key = ApiKey::new(KEY).on_reject(move |_: Request| {
        let flag = flag.clone();
        async move {
            tokio::task::yield_now().await;
            flag.store(true, Ordering::SeqCst);
        }
    });
    let router = Router::new()
        .fallback(any(|| async { StatusCode::NO_CONTENT }))
        .layer(from_fn_with_state(key, require_api_key));
    let response = router.oneshot(request("/any", None)).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(done.load(Ordering::SeqCst));
}

#[test]
fn debug_does_not_show_the_key() {
    assert!(!format!("{:?}", ApiKey::new(KEY)).contains(KEY));
}
