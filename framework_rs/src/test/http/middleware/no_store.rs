use axum::extract::Request;
use axum::http::header;

use crate::http::middleware::no_store::*;
use axum::body::Body;
use axum::middleware::from_fn;
use axum::routing::get;
use axum::Router;
use tower::ServiceExt;

async fn cache_control(router: Router) -> String {
    let request = Request::builder().uri("/").body(Body::empty()).unwrap();
    let response = router
        .layer(from_fn(no_store))
        .oneshot(request)
        .await
        .unwrap();
    response.headers()[header::CACHE_CONTROL]
        .to_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn sets_no_store() {
    let router = Router::new().route("/", get(|| async { "ok" }));
    assert_eq!(cache_control(router).await, "no-store");
}

#[tokio::test]
async fn keeps_the_cache_control_of_the_handler() {
    let router = Router::new().route(
        "/",
        get(|| async { ([(header::CACHE_CONTROL, "private, max-age=60")], "ok") }),
    );
    assert_eq!(cache_control(router).await, "private, max-age=60");
}
