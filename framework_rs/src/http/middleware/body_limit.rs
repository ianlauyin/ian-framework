use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use http_body_util::Limited;

use crate::exception;
use crate::http::error_code;

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
    exception!(
        "request body is too large",
        code = error_code::PAYLOAD_TOO_LARGE
    )
    .into_response()
}
