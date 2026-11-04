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
