use std::time::Duration;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::exception;
use crate::http::error_code;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestTimeout(pub Duration);

pub async fn request_timeout(
    State(RequestTimeout(limit)): State<RequestTimeout>,
    request: Request,
    next: Next,
) -> Response {
    match tokio::time::timeout(limit, next.run(request)).await {
        Ok(response) => response,
        Err(_) => {
            exception!(code = error_code::REQUEST_TIMEOUT, "request timed out").into_response()
        }
    }
}
