use super::ApiError;

pub trait IntoApiError {
    fn status_code(&self) -> u16;
    fn error_code(&self) -> &str;
    fn message(&self) -> String;
}

impl<E: IntoApiError> From<E> for ApiError {
    fn from(error: E) -> Self {
        ApiError::from_status_code(error.status_code(), error.error_code(), error.message())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    enum DemoError {
        Missing,
        BadStatus,
    }

    impl IntoApiError for DemoError {
        fn status_code(&self) -> u16 {
            match self {
                DemoError::Missing => 404,
                DemoError::BadStatus => 0,
            }
        }

        fn error_code(&self) -> &str {
            match self {
                DemoError::Missing => "not_found",
                DemoError::BadStatus => "bad",
            }
        }

        fn message(&self) -> String {
            match self {
                DemoError::Missing => "missing".to_string(),
                DemoError::BadStatus => "bad status".to_string(),
            }
        }
    }

    fn load_missing() -> Result<String, DemoError> {
        Err(DemoError::Missing)
    }

    fn read_item() -> Result<String, ApiError> {
        let item = load_missing()?;
        Ok(item)
    }

    #[test]
    fn question_mark_and_map_err_convert_domain_error() {
        let from_question = read_item().unwrap_err();
        assert_eq!(from_question.status, StatusCode::NOT_FOUND);
        assert_eq!(from_question.code, "not_found");
        assert_eq!(from_question.message, "missing");

        let from_map = load_missing().map_err(ApiError::from).unwrap_err();
        assert_eq!(from_map, from_question);
    }

    #[tokio::test]
    async fn from_domain_error_sets_status_and_json_body() {
        let error = ApiError::from(DemoError::Missing);
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            "application/json"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], br#"{"code":"not_found","message":"missing"}"#);

        let bad = ApiError::from(DemoError::BadStatus);
        assert_eq!(bad.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(bad.code, "bad");
        assert_eq!(bad.message, "bad status");
        assert_eq!(
            bad.into_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
