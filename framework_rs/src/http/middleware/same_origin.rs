use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::http::headers::is_same_origin;
use crate::http::ApiError;

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
