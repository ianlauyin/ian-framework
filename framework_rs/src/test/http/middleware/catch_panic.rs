use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::Response;

use crate::http::middleware::catch_panic::*;
use axum::body::{to_bytes, Body};
use axum::middleware::from_fn;
use axum::routing::get;
use axum::Router;
use tower::ServiceExt;

async fn send(path: &str) -> Response {
    let router = Router::new()
        .route("/ok", get(|| async { "fine" }))
        .route(
            "/panic",
            get(|| async {
                if true {
                    panic!("boom with a secret");
                }
                "unreachable"
            }),
        )
        .route(
            "/late-panic",
            get(|| async {
                tokio::task::yield_now().await;
                if true {
                    panic!("late boom");
                }
                "unreachable"
            }),
        )
        .layer(from_fn(catch_panic));
    let request = Request::builder().uri(path).body(Body::empty()).unwrap();
    router.oneshot(request).await.unwrap()
}

#[tokio::test]
async fn passes_a_normal_response() {
    let response = send("/ok").await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn turns_a_panic_into_a_500_without_the_detail() {
    for path in ["/panic", "/late-panic"] {
        let response = send(path).await;
        assert_eq!(
            response.status(),
            StatusCode::INTERNAL_SERVER_ERROR,
            "{path}"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            body, r#"{"code":"internal_error","message":"internal error"}"#,
            "{path}"
        );
    }
}
