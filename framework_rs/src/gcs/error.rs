use std::fmt;

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

pub(super) fn api_detail(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    if let Some(message) = value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(|m| m.as_str())
    {
        return Some(message.to_string());
    }
    if let Some(message) = value.get("error_description").and_then(|m| m.as_str()) {
        return Some(message.to_string());
    }
    None
}

pub(super) fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_detail_reads_the_gcs_message() {
        let body = r#"{"error":{"code":404,"message":"No such object"}}"#;
        assert_eq!(api_detail(body).as_deref(), Some("No such object"));
    }

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
