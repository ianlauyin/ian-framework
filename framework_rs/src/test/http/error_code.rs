use axum::http::header;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;

use crate::exception;
use crate::http::error_code;

async fn bytes(response: Response) -> Vec<u8> {
    axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
}

async fn message(response: Response) -> String {
    let body = bytes(response).await;
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(value.get("error").is_none());
    assert_eq!(value.as_object().unwrap().len(), 1);
    value["message"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn a_well_known_code_gives_its_status_and_message() {
    for (error, status) in [
        (
            exception!(code = error_code::VALIDATION_ERROR, "name is blank"),
            StatusCode::BAD_REQUEST,
        ),
        (
            exception!(code = error_code::BAD_REQUEST, "bad query"),
            StatusCode::BAD_REQUEST,
        ),
        (
            exception!(code = error_code::UNAUTHORIZED, "unauthorized"),
            StatusCode::UNAUTHORIZED,
        ),
        (
            exception!(code = error_code::FORBIDDEN, "access denied"),
            StatusCode::FORBIDDEN,
        ),
        (
            exception!(code = error_code::NOT_FOUND, "job not found"),
            StatusCode::NOT_FOUND,
        ),
        (
            exception!(code = error_code::REQUEST_TIMEOUT, "request timed out"),
            StatusCode::REQUEST_TIMEOUT,
        ),
        (
            exception!(
                code = error_code::PAYLOAD_TOO_LARGE,
                "request body is too large",
            ),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
        (
            exception!(code = error_code::TOO_MANY_REQUESTS, "too many requests"),
            StatusCode::TOO_MANY_REQUESTS,
        ),
        (
            exception!(code = error_code::INTERNAL_ERROR, "internal error"),
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    ] {
        let text = error.message().to_owned();
        let response = error.into_response();
        assert_eq!(response.status(), status);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            "application/json"
        );
        assert_eq!(message(response).await, text);
    }
}

#[tokio::test]
async fn a_different_code_is_a_hidden_500() {
    for error in [
        exception!(code = "MY_CODE", "db down"),
        exception!(code = "GCS_NOT_FOUND", "secret detail"),
        exception!(code = "EXTERNAL_ERROR", "external"),
    ] {
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(message(response).await, "internal server error");
    }
}

#[tokio::test]
async fn only_the_code_of_the_top_exception_counts() {
    let inner = exception!(code = error_code::VALIDATION_ERROR, "name is blank");
    let outer = exception!(code = "MY_CODE", "secret message", source = inner);
    let response = outer.into_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(message(response).await, "internal server error");

    let wrapped = exception!(
        code = "MY_CODE",
        "secret message",
        source = exception!(code = error_code::VALIDATION_ERROR, "x"),
    );
    let response = wrapped.into_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(message(response).await, "internal server error");
}

#[tokio::test]
async fn into_response_encodes_json_strings() {
    let response = exception!(code = error_code::BAD_REQUEST, "line\n\"x").into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = bytes(response).await;
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["message"], "line\n\"x");
    assert!(value.get("error").is_none());
    assert_eq!(value.as_object().unwrap().len(), 1);
}
