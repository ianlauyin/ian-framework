use std::fmt;

use axum::http::StatusCode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn from_status_code(
        status: u16,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            code,
            message,
        )
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_keeps_status_code_and_message() {
        let error = ApiError::new(StatusCode::BAD_REQUEST, "bad_input", "nope");
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.code, "bad_input");
        assert_eq!(error.message, "nope");
        assert_eq!(error.to_string(), "bad_input: nope");
    }

    #[test]
    fn from_status_code_keeps_status_from_100_through_999() {
        let error = ApiError::from_status_code(404, "not_found", "missing");
        assert_eq!(error.status, StatusCode::NOT_FOUND);
        assert_eq!(error.code, "not_found");
        assert_eq!(error.message, "missing");
        assert_eq!(
            ApiError::from_status_code(100, "cont", "wait").status,
            StatusCode::CONTINUE
        );
        assert_eq!(
            ApiError::from_status_code(500, "internal", "failed").status,
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            ApiError::from_status_code(999, "custom", "custom")
                .status
                .as_u16(),
            999
        );
    }

    #[test]
    fn from_status_code_uses_500_for_invalid_status() {
        for status in [0, 99, 1000] {
            let error = ApiError::from_status_code(status, "bad", "nope");
            assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(error.code, "bad");
            assert_eq!(error.message, "nope");
        }
    }
}
