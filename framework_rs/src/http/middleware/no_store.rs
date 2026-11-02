use axum::extract::Request;
use axum::http::{header, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;

pub async fn no_store(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    if !response.headers().contains_key(header::CACHE_CONTROL) {
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
