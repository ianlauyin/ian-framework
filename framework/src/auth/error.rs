use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthErrorKind {
    Config,
    Store,
    Password,
    Random,
}

pub struct AuthError {
    kind: AuthErrorKind,
    message: String,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl AuthError {
    pub fn kind(&self) -> AuthErrorKind {
        self.kind
    }

    pub(crate) fn new(kind: AuthErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    pub(crate) fn with_source(
        kind: AuthErrorKind,
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
}

impl fmt::Debug for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthError")
            .field("kind", &self.kind)
            .field("message", &self.message)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for AuthError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.source {
            Some(source) => Some(source.as_ref()),
            None => None,
        }
    }
}

impl crate::http::IntoApiError for AuthError {
    fn status_code(&self) -> u16 {
        match self.kind {
            AuthErrorKind::Store => 503,
            AuthErrorKind::Config | AuthErrorKind::Password | AuthErrorKind::Random => 500,
        }
    }

    fn error_code(&self) -> &str {
        match self.kind {
            AuthErrorKind::Config => "auth_config",
            AuthErrorKind::Store => "auth_store",
            AuthErrorKind::Password => "auth_password",
            AuthErrorKind::Random => "auth_random",
        }
    }

    fn message(&self) -> String {
        let message = match self.kind {
            AuthErrorKind::Config | AuthErrorKind::Store => "session store is not available",
            AuthErrorKind::Password => "password hash failed",
            AuthErrorKind::Random => "session token failed",
        };
        message.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::ApiError;
    use axum::http::StatusCode;

    #[test]
    fn api_error_hides_the_internal_message() {
        let cases = [
            (
                AuthErrorKind::Config,
                StatusCode::INTERNAL_SERVER_ERROR,
                "auth_config",
            ),
            (
                AuthErrorKind::Store,
                StatusCode::SERVICE_UNAVAILABLE,
                "auth_store",
            ),
            (
                AuthErrorKind::Password,
                StatusCode::INTERNAL_SERVER_ERROR,
                "auth_password",
            ),
            (
                AuthErrorKind::Random,
                StatusCode::INTERNAL_SERVER_ERROR,
                "auth_random",
            ),
        ];
        for (kind, status, code) in cases {
            let error = ApiError::from(AuthError::new(kind, "secret redis detail"));
            assert_eq!(error.status, status);
            assert_eq!(error.code, code);
            assert!(!error.message.contains("secret"));
        }
    }
}
