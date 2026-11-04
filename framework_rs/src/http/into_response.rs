use axum::body::Body;
use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};

use super::ApiError;

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status;
        let body = serde_json::json!({
            "code": self.code,
            "message": self.message,
        })
        .to_string();
        (
            status,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )],
            Body::from(body),
        )
            .into_response()
    }
}
