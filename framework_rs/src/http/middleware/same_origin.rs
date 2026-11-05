use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::exception;
use crate::http::error_code;
use crate::http::headers::is_same_origin;

pub async fn require_same_origin(request: Request, next: Next) -> Response {
    if !is_same_origin(request.headers()) {
        return exception!("cross-origin request", code = error_code::FORBIDDEN).into_response();
    }
    next.run(request).await
}
