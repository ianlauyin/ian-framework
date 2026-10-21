use axum::body::Body;
use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use super::ApiError;

#[derive(Serialize)]
struct ApiErrorBody {
    code: String,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status;
        let bytes = serde_json::to_vec(&ApiErrorBody {
            code: self.code,
            message: self.message,
        })
        .expect("api error json");
        (
            status,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )],
            Body::from(bytes),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    async fn bytes(response: Response) -> Vec<u8> {
        axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec()
    }

    #[tokio::test]
    async fn into_response_sets_status_and_flat_json_body() {
        let response = ApiError::new(StatusCode::NOT_FOUND, "not_found", "missing").into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            "application/json"
        );
        let body = bytes(response).await;
        assert_eq!(body, br#"{"code":"not_found","message":"missing"}"#);
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(value.get("error").is_none());
        assert_eq!(value.as_object().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn into_response_encodes_json_strings() {
        let response =
            ApiError::new(StatusCode::BAD_REQUEST, "bad\"code", "line\n").into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = bytes(response).await;
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["code"], "bad\"code");
        assert_eq!(value["message"], "line\n");
        assert!(value.get("error").is_none());
        assert_eq!(value.as_object().unwrap().len(), 2);
    }
}
