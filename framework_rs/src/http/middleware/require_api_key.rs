use std::fmt;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::exception;
use crate::http::error_code;
use crate::http::headers::bearer::{bearer_token, key_matches};

type OnReject = Arc<dyn Fn(&Request) + Send + Sync>;

#[derive(Clone)]
pub struct ApiKey {
    key: Arc<str>,
    on_reject: Option<OnReject>,
}

impl ApiKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into().into(),
            on_reject: None,
        }
    }

    pub fn on_reject(mut self, callback: impl Fn(&Request) + Send + Sync + 'static) -> Self {
        self.on_reject = Some(Arc::new(callback));
        self
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey([redacted])")
    }
}

pub async fn require_api_key(State(key): State<ApiKey>, request: Request, next: Next) -> Response {
    let allowed = bearer_token(request.headers()).is_some_and(|token| key_matches(token, &key.key));
    if !allowed {
        if let Some(on_reject) = &key.on_reject {
            on_reject(&request);
        }
        return exception!(code = error_code::UNAUTHORIZED, "unauthorized").into_response();
    }
    next.run(request).await
}
