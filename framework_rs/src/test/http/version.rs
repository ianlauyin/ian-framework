use axum::http::StatusCode;
use axum::Router;

use crate::http::version::*;
use axum::body::{to_bytes, Body};
use axum::http::Request;
use tower::ServiceExt;

async fn read_version(router: Router) -> String {
    let response = router
        .oneshot(
            Request::builder()
                .uri("/version")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "application/json");
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();
    body.strip_prefix("{\"version\":\"")
        .and_then(|rest| rest.strip_suffix("\"}"))
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn version_stays_the_same_for_the_process() {
    let first = read_version(version_router()).await;
    let second = read_version(version_router()).await;
    assert_eq!(first.len(), 32);
    assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(first, second);
}

#[tokio::test]
async fn other_paths_are_not_found() {
    let response = version_router()
        .oneshot(
            Request::builder()
                .uri("/other")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
