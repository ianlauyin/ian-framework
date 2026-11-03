use std::fmt;

use axum::http::StatusCode;

use crate::exception::error_code;
use crate::exception::Exception;

const INTERNAL_CODE: &str = "INTERNAL_ERROR";
const INTERNAL_MESSAGE: &str = "internal error";

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

// Only the code of the top exception counts. A wrapped client error becomes an internal error.
impl From<Exception> for ApiError {
    fn from(exception: Exception) -> Self {
        let Some(code) = exception.code() else {
            return internal_error();
        };
        let status = match code {
            error_code::VALIDATION_ERROR | error_code::BAD_REQUEST => StatusCode::BAD_REQUEST,
            error_code::FORBIDDEN => StatusCode::FORBIDDEN,
            error_code::NOT_FOUND => StatusCode::NOT_FOUND,
            _ => return internal_error(),
        };
        ApiError::new(status, code, exception.message())
    }
}

fn internal_error() -> ApiError {
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        INTERNAL_CODE,
        INTERNAL_MESSAGE,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exception;
    use crate::validation_error;

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

    #[test]
    fn a_well_known_code_gives_its_status_code_and_message() {
        for (error, status) in [
            (validation_error!("name is blank"), StatusCode::BAD_REQUEST),
            (
                exception!("bad query", code = error_code::BAD_REQUEST),
                StatusCode::BAD_REQUEST,
            ),
            (
                exception!("access denied", code = error_code::FORBIDDEN),
                StatusCode::FORBIDDEN,
            ),
            (
                exception!("job not found", code = error_code::NOT_FOUND),
                StatusCode::NOT_FOUND,
            ),
        ] {
            let code = error.code().unwrap();
            let message = error.message().to_owned();
            let api = ApiError::from(error);
            assert_eq!(api.status, status);
            assert_eq!(api.code, code);
            assert_eq!(api.message, message);
        }
    }

    #[test]
    fn a_different_code_or_no_code_is_a_hidden_500() {
        for error in [
            exception!("db down", code = "MY_CODE"),
            exception!("secret detail", code = "GCS_NOT_FOUND"),
            exception!("no code"),
        ] {
            let api = ApiError::from(error);
            assert_eq!(api.status, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(api.code, "INTERNAL_ERROR");
            assert_eq!(api.message, "internal error");
        }
    }

    #[test]
    fn only_the_code_of_the_top_exception_counts() {
        let inner = validation_error!("name is blank");
        let outer = exception!("secret message", code = "MY_CODE", source = inner);
        let api = ApiError::from(outer);
        assert_eq!(api.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(api.message, "internal error");
        let wrapped = exception!("secret message", source = validation_error!("x"));
        assert_eq!(ApiError::from(wrapped).code, "INTERNAL_ERROR");
    }
}
