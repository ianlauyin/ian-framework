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
        let code = exception.code();
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
