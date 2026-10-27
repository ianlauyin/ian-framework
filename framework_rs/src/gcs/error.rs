use std::fmt;

use crate::gcp::{api_detail, truncate, GcpError, GcpErrorKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcsErrorKind {
    Auth,
    Config,
    NotFound,
    Api,
    Transport,
}

pub struct GcsError {
    kind: GcsErrorKind,
    status: Option<u16>,
    message: String,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl GcsError {
    pub fn kind(&self) -> GcsErrorKind {
        self.kind
    }

    pub(crate) fn status(&self) -> Option<u16> {
        self.status
    }

    pub(crate) fn is_retryable(&self) -> bool {
        if self.kind == GcsErrorKind::Transport {
            return true;
        }
        matches!(self.status, Some(408 | 429 | 500 | 502 | 503 | 504))
    }

    pub(crate) fn new(kind: GcsErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            status: None,
            message: message.into(),
            source: None,
        }
    }

    pub(crate) fn with_status(kind: GcsErrorKind, status: u16, message: impl Into<String>) -> Self {
        Self {
            kind,
            status: Some(status),
            message: message.into(),
            source: None,
        }
    }

    pub(crate) fn with_source(
        kind: GcsErrorKind,
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            status: None,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    pub(crate) async fn from_response(response: reqwest::Response) -> Self {
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        let detail = api_detail(&text).unwrap_or(text);
        let detail = truncate(&detail, 500);
        let kind = match status {
            401 | 403 => GcsErrorKind::Auth,
            404 => GcsErrorKind::NotFound,
            _ => GcsErrorKind::Api,
        };
        Self::with_status(kind, status, format!("status {status}: {detail}"))
    }
}

impl fmt::Debug for GcsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GcsError")
            .field("kind", &self.kind)
            .field("status", &self.status)
            .field("message", &self.message)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for GcsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for GcsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.source {
            Some(source) => Some(source.as_ref()),
            None => None,
        }
    }
}

impl From<GcpError> for GcsError {
    fn from(err: GcpError) -> Self {
        let kind = match err.kind() {
            GcpErrorKind::Auth => GcsErrorKind::Auth,
            GcpErrorKind::Config => GcsErrorKind::Config,
            GcpErrorKind::Api => GcsErrorKind::Api,
            GcpErrorKind::Transport => GcsErrorKind::Transport,
        };
        let (message, source) = err.into_parts();
        Self {
            kind,
            status: None,
            message,
            source,
        }
    }
}

impl crate::http::IntoApiError for GcsError {
    fn status_code(&self) -> u16 {
        match self.kind {
            GcsErrorKind::NotFound => 404,
            GcsErrorKind::Auth | GcsErrorKind::Config => 500,
            GcsErrorKind::Api | GcsErrorKind::Transport => 502,
        }
    }

    fn error_code(&self) -> &str {
        match self.kind {
            GcsErrorKind::NotFound => "gcs_not_found",
            GcsErrorKind::Auth => "gcs_auth",
            GcsErrorKind::Config => "gcs_config",
            GcsErrorKind::Api => "gcs_api",
            GcsErrorKind::Transport => "gcs_transport",
        }
    }

    fn message(&self) -> String {
        let message = match self.kind {
            GcsErrorKind::NotFound => "storage object not found",
            GcsErrorKind::Auth | GcsErrorKind::Config => "storage is not available",
            GcsErrorKind::Api | GcsErrorKind::Transport => "storage request failed",
        };
        message.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retryable_status_codes_match_the_upload_policy() {
        assert!(GcsError::new(GcsErrorKind::Transport, "reset").is_retryable());
        for status in [408_u16, 429, 500, 502, 503, 504] {
            assert!(GcsError::with_status(GcsErrorKind::Api, status, "retry").is_retryable());
        }
        assert!(!GcsError::with_status(GcsErrorKind::NotFound, 404, "missing").is_retryable());
        assert!(!GcsError::with_status(GcsErrorKind::Api, 400, "bad").is_retryable());
        assert!(!GcsError::new(GcsErrorKind::Config, "config").is_retryable());
    }

    #[test]
    fn api_error_hides_the_internal_message() {
        use crate::http::ApiError;
        use axum::http::StatusCode;

        let cases = [
            (
                GcsErrorKind::NotFound,
                StatusCode::NOT_FOUND,
                "gcs_not_found",
            ),
            (
                GcsErrorKind::Auth,
                StatusCode::INTERNAL_SERVER_ERROR,
                "gcs_auth",
            ),
            (
                GcsErrorKind::Config,
                StatusCode::INTERNAL_SERVER_ERROR,
                "gcs_config",
            ),
            (GcsErrorKind::Api, StatusCode::BAD_GATEWAY, "gcs_api"),
            (
                GcsErrorKind::Transport,
                StatusCode::BAD_GATEWAY,
                "gcs_transport",
            ),
        ];
        for (kind, status, code) in cases {
            let error = ApiError::from(GcsError::new(kind, "secret bucket detail"));
            assert_eq!(error.status, status);
            assert_eq!(error.code, code);
            assert!(!error.message.contains("secret"));
        }

        let error = ApiError::from(GcsError::with_status(GcsErrorKind::Api, 400, "bad"));
        assert_eq!(error.status, StatusCode::BAD_GATEWAY);
        assert_eq!(error.message, "storage request failed");
    }
}
