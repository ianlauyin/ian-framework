use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use http_body_util::Limited;

use crate::http::ApiError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyLimit(pub usize);

pub async fn body_limit(
    State(BodyLimit(max)): State<BodyLimit>,
    request: Request,
    next: Next,
) -> Response {
    if declared_length(request.headers()).is_some_and(|length| length > max) {
        return too_large();
    }
    let (parts, body) = request.into_parts();
    let request = Request::from_parts(parts, Body::new(Limited::new(body, max)));
    let response = next.run(request).await;
    if response.status() == StatusCode::PAYLOAD_TOO_LARGE && !is_json(response.headers()) {
        return too_large();
    }
    response
}

fn declared_length(headers: &HeaderMap) -> Option<usize> {
    headers
        .get(header::CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn is_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("application/json"))
}

fn too_large() -> Response {
    ApiError::new(
        StatusCode::PAYLOAD_TOO_LARGE,
        "payload_too_large",
        "request body is too large",
    )
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Bytes};
    use axum::extract::DefaultBodyLimit;
    use axum::middleware::from_fn_with_state;
    use axum::routing::post;
    use axum::Router;
    use tower::ServiceExt;

    const TOO_LARGE: &str = r#"{"code":"payload_too_large","message":"request body is too large"}"#;

    async fn echo_len(body: Bytes) -> String {
        body.len().to_string()
    }

    fn router(max: usize) -> Router {
        Router::new()
            .route("/", post(echo_len))
            .layer(DefaultBodyLimit::disable())
            .layer(from_fn_with_state(BodyLimit(max), body_limit))
    }

    fn post_request(body: Body, length: Option<&str>) -> Request {
        let mut request = Request::builder().method("POST").uri("/");
        if let Some(length) = length {
            request = request.header("content-length", length);
        }
        request.body(body).unwrap()
    }

    async fn text(response: Response) -> String {
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(body.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn passes_a_body_within_the_limit() {
        let request = post_request(Body::from("12345678"), Some("8"));
        let response = router(8).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(text(response).await, "8");
    }

    #[tokio::test]
    async fn rejects_a_declared_length_over_the_limit() {
        let request = post_request(Body::from("123456789"), Some("9"));
        let response = router(8).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(text(response).await, TOO_LARGE);
    }

    #[tokio::test]
    async fn rejects_a_body_over_the_limit_with_no_declared_length() {
        let request = post_request(Body::from("123456789"), None);
        let response = router(8).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(text(response).await, TOO_LARGE);
    }

    #[tokio::test]
    async fn rejects_a_body_that_is_larger_than_its_declared_length() {
        let request = post_request(Body::from("123456789"), Some("4"));
        let response = router(8).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(text(response).await, TOO_LARGE);
    }
}
