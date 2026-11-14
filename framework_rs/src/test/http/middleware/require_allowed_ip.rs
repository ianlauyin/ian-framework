use axum::body::{to_bytes, Body};
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::from_fn_with_state;
use axum::routing::any;
use axum::Router;
use tower::ServiceExt;

use crate::http::middleware::require_allowed_ip::*;

fn router(ips: &[&str]) -> Router {
    Router::new()
        .fallback(any(|| async { StatusCode::NO_CONTENT }))
        .layer(from_fn_with_state(
            AllowedIps::new(ips.iter().copied()).unwrap(),
            require_allowed_ip,
        ))
}

fn request(ip: Option<&str>) -> Request {
    let mut request = Request::builder().uri("/any");
    if let Some(ip) = ip {
        request = request.header("cf-connecting-ip", ip);
    }
    request.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn passes_a_request_from_an_allowed_ip() {
    let response = router(&["192.0.2.1", "198.51.100.1"])
        .oneshot(request(Some("192.0.2.1")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn rejects_a_missing_or_disallowed_ip_with_403() {
    let router = router(&["192.0.2.1"]);
    for ip in [None, Some("198.51.100.1"), Some("not-an-ip"), Some("")] {
        let response = router.clone().oneshot(request(ip)).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}

#[tokio::test]
async fn rejects_every_request_when_the_set_is_empty() {
    let response = router(&[])
        .oneshot(request(Some("192.0.2.1")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_rejection_body_is_json() {
    let response = router(&["192.0.2.1"]).oneshot(request(None)).await.unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body, r#"{"message":"ip not allowed"}"#);
}

#[test]
fn new_rejects_an_invalid_ip() {
    let err = AllowedIps::new(["192.0.2.1", "not-an-ip"]).unwrap_err();
    assert_eq!(err.code(), ALLOWED_IPS_INVALID);
}
