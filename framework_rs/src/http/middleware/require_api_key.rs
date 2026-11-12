use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::exception;
use crate::http::error_code;
use crate::http::headers::bearer::{bearer_token, key_matches};
use crate::logging::Logger;

#[derive(Clone)]
pub struct ApiKey(Arc<str>);

impl ApiKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into().into())
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey([redacted])")
    }
}

pub async fn require_api_key(State(key): State<ApiKey>, request: Request, next: Next) -> Response {
    let allowed = bearer_token(request.headers()).is_some_and(|token| key_matches(token, &key.0));
    if !allowed {
        Logger::warn(
            "HTTP/apiKeyRejected",
            BTreeMap::from([("method".to_string(), request.method().as_str().into())]),
        );
        return exception!(code = error_code::UNAUTHORIZED, "unauthorized").into_response();
    }
    next.run(request).await
}
