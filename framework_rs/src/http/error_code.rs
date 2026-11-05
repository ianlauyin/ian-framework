use axum::body::Body;
use axum::http::header;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;

use crate::exception::Exception;

pub const VALIDATION_ERROR: &str = "VALIDATION_ERROR";
pub const BAD_REQUEST: &str = "BAD_REQUEST";
pub const UNAUTHORIZED: &str = "UNAUTHORIZED";
pub const FORBIDDEN: &str = "FORBIDDEN";
pub const NOT_FOUND: &str = "NOT_FOUND";
pub const REQUEST_TIMEOUT: &str = "REQUEST_TIMEOUT";
pub const PAYLOAD_TOO_LARGE: &str = "PAYLOAD_TOO_LARGE";
pub const TOO_MANY_REQUESTS: &str = "TOO_MANY_REQUESTS";
pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";

// Only the code of the top exception counts. A wrapped client error becomes an internal error.
impl IntoResponse for Exception {
    fn into_response(self) -> Response {
        let code = match self.code() {
            VALIDATION_ERROR | BAD_REQUEST => StatusCode::BAD_REQUEST,
            UNAUTHORIZED => StatusCode::UNAUTHORIZED,
            FORBIDDEN => StatusCode::FORBIDDEN,
            NOT_FOUND => StatusCode::NOT_FOUND,
            REQUEST_TIMEOUT => StatusCode::REQUEST_TIMEOUT,
            PAYLOAD_TOO_LARGE => StatusCode::PAYLOAD_TOO_LARGE,
            TOO_MANY_REQUESTS => StatusCode::TOO_MANY_REQUESTS,
            INTERNAL_ERROR => StatusCode::INTERNAL_SERVER_ERROR,
            _ => {
                return error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal server error");
            }
        };
        error_response(code, self.message())
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
