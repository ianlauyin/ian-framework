use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::{is_same_origin, ApiError};

pub async fn require_same_origin(request: Request, next: Next) -> Response {
    if !is_same_origin(request.headers()) {
        return ApiError::new(
            StatusCode::FORBIDDEN,
            "cross_origin",
            "cross-origin request",
        )
        .into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::middleware::from_fn;
    use axum::routing::post;
    use axum::Router;
    use tower::ServiceExt;

    fn router() -> Router {
        Router::new()
            .route("/items", post(|| async { StatusCode::NO_CONTENT }))
            .layer(from_fn(require_same_origin))
    }

    async fn send(headers: &[(&str, &str)]) -> Response {
        let mut request = Request::builder().method("POST").uri("/items");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        router()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn passes_a_same_origin_request() {
        let response = send(&[("sec-fetch-site", "same-origin")]).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let response = send(&[("origin", "https://app.example"), ("host", "app.example")]).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn rejects_a_cross_origin_request_with_403() {
        for headers in [
            &[("sec-fetch-site", "cross-site")][..],
            &[("sec-fetch-site", "same-site")][..],
            &[("origin", "https://evil.example"), ("host", "app.example")][..],
            &[][..],
        ] {
            let response = send(headers).await;
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            assert_eq!(
                body,
                r#"{"code":"cross_origin","message":"cross-origin request"}"#
            );
        }
    }
}
