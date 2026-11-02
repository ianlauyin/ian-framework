use std::fmt;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::http::headers::{bearer_token, key_matches};
use crate::http::ApiError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathMatch {
    Exact(String),
    Tree(String),
    Child(String),
}

#[derive(Clone)]
pub struct ApiKeyRule {
    path: PathMatch,
    methods: Option<Vec<Method>>,
    key: String,
}

#[derive(Clone)]
pub struct ApiKeyRules(Arc<Vec<ApiKeyRule>>);

impl ApiKeyRule {
    pub fn new(path: PathMatch, key: impl Into<String>) -> Self {
        Self {
            path,
            methods: None,
            key: key.into(),
        }
    }

    pub fn methods(mut self, methods: impl IntoIterator<Item = Method>) -> Self {
        self.methods = Some(methods.into_iter().collect());
        self
    }

    fn applies(&self, method: &Method, path: &str) -> bool {
        if let Some(methods) = &self.methods {
            if !methods.contains(method) {
                return false;
            }
        }
        match &self.path {
            PathMatch::Exact(exact) => path == exact,
            PathMatch::Tree(root) => {
                path == root
                    || path
                        .strip_prefix(root.as_str())
                        .is_some_and(|rest| rest.starts_with('/'))
            }
            PathMatch::Child(root) => path
                .strip_prefix(root.as_str())
                .and_then(|rest| rest.strip_prefix('/'))
                .is_some_and(|name| !name.is_empty() && !name.contains('/')),
        }
    }
}

impl ApiKeyRules {
    pub fn new(rules: Vec<ApiKeyRule>) -> Self {
        Self(Arc::new(rules))
    }
}

impl fmt::Debug for ApiKeyRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiKeyRule")
            .field("path", &self.path)
            .field("methods", &self.methods)
            .field("key", &"[redacted]")
            .finish()
    }
}

impl fmt::Debug for ApiKeyRules {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ApiKeyRules").field(&self.0).finish()
    }
}

pub async fn require_api_key(
    State(rules): State<ApiKeyRules>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method();
    let path = request.uri().path();
    let Some(rule) = rules.0.iter().find(|rule| rule.applies(method, path)) else {
        return next.run(request).await;
    };
    let allowed =
        bearer_token(request.headers()).is_some_and(|token| key_matches(token, &rule.key));
    if !allowed {
        tracing::warn!(method = %method, "rejected request without a valid api key");
        return ApiError::new(StatusCode::UNAUTHORIZED, "unauthorized", "unauthorized")
            .into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::middleware::from_fn_with_state;
    use axum::routing::any;
    use axum::Router;
    use tower::ServiceExt;

    const READ_KEY: &str = "read-key-value";
    const WRITE_KEY: &str = "write-key-value";
    const OPS_KEY: &str = "ops-key-value";

    fn router() -> Router {
        let rules = ApiKeyRules::new(vec![
            ApiKeyRule::new(PathMatch::Tree("/ops".into()), OPS_KEY),
            ApiKeyRule::new(PathMatch::Exact("/log".into()), READ_KEY).methods([Method::GET]),
            ApiKeyRule::new(PathMatch::Child("/log".into()), READ_KEY).methods([Method::GET]),
            ApiKeyRule::new(PathMatch::Child("/log".into()), WRITE_KEY).methods([Method::POST]),
        ]);
        Router::new()
            .fallback(any(|| async { StatusCode::NO_CONTENT }))
            .layer(from_fn_with_state(rules, require_api_key))
    }

    async fn status(method: Method, path: &str, key: Option<&str>) -> StatusCode {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(key) = key {
            request = request.header("authorization", format!("Bearer {key}"));
        }
        router()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn uses_the_key_of_the_matching_rule() {
        let no_content = StatusCode::NO_CONTENT;
        assert_eq!(
            status(Method::POST, "/log/my-app", Some(WRITE_KEY)).await,
            no_content
        );
        assert_eq!(
            status(Method::GET, "/log/my-app", Some(READ_KEY)).await,
            no_content
        );
        assert_eq!(
            status(Method::GET, "/log", Some(READ_KEY)).await,
            no_content
        );
        assert_eq!(
            status(Method::POST, "/ops/archive", Some(OPS_KEY)).await,
            no_content
        );
        assert_eq!(
            status(Method::POST, "/ops", Some(OPS_KEY)).await,
            no_content
        );
    }

    #[tokio::test]
    async fn rejects_a_missing_or_wrong_key_with_401() {
        let rejected = StatusCode::UNAUTHORIZED;
        assert_eq!(status(Method::POST, "/log/my-app", None).await, rejected);
        assert_eq!(
            status(Method::POST, "/log/my-app", Some(READ_KEY)).await,
            rejected
        );
        assert_eq!(
            status(Method::GET, "/log/my-app", Some(WRITE_KEY)).await,
            rejected
        );
        assert_eq!(
            status(Method::POST, "/ops/archive", Some(WRITE_KEY)).await,
            rejected
        );
        assert_eq!(status(Method::GET, "/log", Some("wrong")).await, rejected);
    }

    #[tokio::test]
    async fn the_rejection_body_is_json() {
        let request = Request::builder()
            .uri("/ops/archive")
            .body(Body::empty())
            .unwrap();
        let response = router().oneshot(request).await.unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body, r#"{"code":"unauthorized","message":"unauthorized"}"#);
    }

    #[tokio::test]
    async fn passes_a_request_that_no_rule_matches() {
        let no_content = StatusCode::NO_CONTENT;
        assert_eq!(status(Method::GET, "/health", None).await, no_content);
        assert_eq!(status(Method::GET, "/operations", None).await, no_content);
        assert_eq!(
            status(Method::GET, "/log/my-app/extra", None).await,
            no_content
        );
        assert_eq!(status(Method::GET, "/log/", None).await, no_content);
        assert_eq!(status(Method::DELETE, "/log", None).await, no_content);
    }

    #[test]
    fn debug_does_not_show_the_key() {
        let rule = ApiKeyRule::new(PathMatch::Exact("/log".into()), OPS_KEY);
        let text = format!("{:?}", ApiKeyRules::new(vec![rule]));
        assert!(!text.contains(OPS_KEY));
    }
}
