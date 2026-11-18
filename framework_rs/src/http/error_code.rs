use std::sync::Arc;

use axum::body::Body;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::IntoResponse;
use axum::response::Response;

use crate::exception::Exception;

pub const VALIDATION_ERROR: &str = "VALIDATION_ERROR";
pub const BAD_REQUEST: &str = "BAD_REQUEST";
pub const UNAUTHORIZED: &str = "UNAUTHORIZED";
pub const FORBIDDEN: &str = "FORBIDDEN";
pub const NOT_FOUND: &str = "NOT_FOUND";
pub const REQUEST_TIMEOUT: &str = "REQUEST_TIMEOUT";
pub const CONFLICT: &str = "CONFLICT";
pub const PAYLOAD_TOO_LARGE: &str = "PAYLOAD_TOO_LARGE";
pub const RANGE_NOT_SATISFIABLE: &str = "RANGE_NOT_SATISFIABLE";
pub const TOO_MANY_REQUESTS: &str = "TOO_MANY_REQUESTS";
pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
pub const BAD_GATEWAY: &str = "BAD_GATEWAY";

impl IntoResponse for Exception {
    fn into_response(self) -> Response {
        let status = match self.code() {
            VALIDATION_ERROR | BAD_REQUEST => Some(StatusCode::BAD_REQUEST),
            UNAUTHORIZED => Some(StatusCode::UNAUTHORIZED),
            FORBIDDEN => Some(StatusCode::FORBIDDEN),
            NOT_FOUND => Some(StatusCode::NOT_FOUND),
            REQUEST_TIMEOUT => Some(StatusCode::REQUEST_TIMEOUT),
            CONFLICT => Some(StatusCode::CONFLICT),
            PAYLOAD_TOO_LARGE => Some(StatusCode::PAYLOAD_TOO_LARGE),
            RANGE_NOT_SATISFIABLE => Some(StatusCode::RANGE_NOT_SATISFIABLE),
            TOO_MANY_REQUESTS => Some(StatusCode::TOO_MANY_REQUESTS),
            BAD_GATEWAY => Some(StatusCode::BAD_GATEWAY),
            _ => None,
        };
        let mut response = match status {
            Some(status) => error_response(status, self.message()),
            None => error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal server error"),
        };
        response.extensions_mut().insert(Arc::new(self));
        response
    }
}

fn error_response(code: StatusCode, message: impl Into<String>) -> Response {
    let body = serde_json::json!({
        "message": message.into(),
    })
    .to_string();
    (
        code,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )],
        Body::from(body),
    )
        .into_response()
}
