use axum::extract::Request;
use axum::http::header;
use axum::response::Response;

use crate::http::middleware::security_headers::*;
use axum::body::Body;
use axum::middleware::from_fn;
use axum::routing::get;
use axum::Router;
use tower::ServiceExt;

async fn send(router: Router) -> Response {
    let request = Request::builder().uri("/").body(Body::empty()).unwrap();
    router
        .layer(from_fn(security_headers))
        .oneshot(request)
        .await
        .unwrap()
}

#[tokio::test]
async fn adds_the_headers() {
    let response = send(Router::new().route("/", get(|| async { "ok" }))).await;
    let headers = response.headers();
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(headers["x-frame-options"], "DENY");
    assert_eq!(headers["referrer-policy"], "no-referrer");
    assert_eq!(
        headers["strict-transport-security"],
        "max-age=31536000; includeSubDomains"
    );
    assert_eq!(
        headers["permissions-policy"],
        "camera=(), microphone=(), geolocation=()"
    );
    assert_eq!(headers["cross-origin-opener-policy"], "same-origin");
    assert!(headers.get("content-security-policy").is_none());
}

#[tokio::test]
async fn keeps_a_header_that_the_handler_set() {
    let router = Router::new().route(
        "/",
        get(|| async { ([(header::X_FRAME_OPTIONS, "SAMEORIGIN")], "ok") }),
    );
    let response = send(router).await;
    assert_eq!(response.headers()["x-frame-options"], "SAMEORIGIN");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
}
