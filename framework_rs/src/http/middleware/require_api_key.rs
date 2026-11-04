use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::http::headers::{bearer_token, key_matches};
use crate::http::ApiError;
use crate::logging::Logger;

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
    // None is a public rule: it matches a request and needs no key.
    key: Option<String>,
}

#[derive(Clone)]
pub struct ApiKeyRules {
    rules: Arc<Vec<ApiKeyRule>>,
    deny_unmatched: bool,
}

impl ApiKeyRule {
    pub fn new(path: PathMatch, key: impl Into<String>) -> Self {
        Self {
            path,
            methods: None,
            key: Some(key.into()),
        }
    }

    // A rule that lets a matching request pass with no key. Use it for a public path when the rules deny unmatched requests.
    pub fn public(path: PathMatch) -> Self {
        Self {
            path,
            methods: None,
            key: None,
        }
    }

    pub fn methods(mut self, methods: impl IntoIterator<Item = Method>) -> Self {
        self.methods = Some(methods.into_iter().collect());
        self
    }

    fn applies(&self, method: &Method, path: &str) -> bool {
        let method_matches = self
            .methods
            .as_ref()
            .is_none_or(|methods| methods.contains(method));
        method_matches && self.path.matches(without_trailing_slash(path))
    }
}

impl ApiKeyRules {
    pub fn new(rules: Vec<ApiKeyRule>) -> Self {
        Self {
            rules: Arc::new(rules),
            deny_unmatched: false,
        }
    }

    // A request that no rule matches gets the 401 response. Without this, it passes.
    pub fn deny_unmatched(mut self) -> Self {
        self.deny_unmatched = true;
        self
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
        f.debug_struct("ApiKeyRules")
            .field("rules", &self.rules)
            .field("deny_unmatched", &self.deny_unmatched)
            .finish()
    }
}

pub async fn require_api_key(
    State(rules): State<ApiKeyRules>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method();
    let path = request.uri().path();
    let rule = rules.rules.iter().find(|rule| rule.applies(method, path));
    let allowed = match rule {
        None => !rules.deny_unmatched,
        Some(rule) => match &rule.key {
            None => true,
            Some(key) => {
                bearer_token(request.headers()).is_some_and(|token| key_matches(token, key))
            }
        },
    };
    if !allowed {
        Logger::warn(
            "HTTP/apiKeyRejected",
            BTreeMap::from([("method".to_string(), method.as_str().to_string())]),
        );
        return ApiError::new(StatusCode::UNAUTHORIZED, "unauthorized", "unauthorized")
            .into_response();
    }
    next.run(request).await
}

impl PathMatch {
    fn matches(&self, path: &str) -> bool {
        match self {
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

// A path with one trailing slash is the same as the path without it, so `/log/` cannot skip a rule for `/log`.
fn without_trailing_slash(path: &str) -> &str {
    match path.strip_suffix('/') {
        Some(trimmed) if !trimmed.is_empty() => trimmed,
        _ => path,
    }
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
        assert_eq!(status(Method::DELETE, "/log", None).await, no_content);
    }

    #[tokio::test]
    async fn a_trailing_slash_does_not_skip_a_rule() {
        let rejected = StatusCode::UNAUTHORIZED;
        assert_eq!(status(Method::GET, "/log/", None).await, rejected);
        assert_eq!(status(Method::POST, "/ops/", None).await, rejected);
        assert_eq!(
            status(Method::GET, "/log/", Some(READ_KEY)).await,
            StatusCode::NO_CONTENT
        );
    }

    #[tokio::test]
    async fn deny_unmatched_refuses_a_path_without_a_rule_except_a_public_one() {
        let rules = ApiKeyRules::new(vec![
            ApiKeyRule::public(PathMatch::Exact("/health".into())),
            ApiKeyRule::new(PathMatch::Tree("/ops".into()), OPS_KEY),
        ])
        .deny_unmatched();
        let router = Router::new()
            .fallback(any(|| async { StatusCode::NO_CONTENT }))
            .layer(from_fn_with_state(rules, require_api_key));
        let get = |path: &str| {
            Request::builder()
                .uri(path.to_string())
                .body(Body::empty())
                .unwrap()
        };
        let health = router.clone().oneshot(get("/health")).await.unwrap();
        assert_eq!(health.status(), StatusCode::NO_CONTENT);
        let other = router.clone().oneshot(get("/other")).await.unwrap();
        assert_eq!(other.status(), StatusCode::UNAUTHORIZED);
        let ops = router.oneshot(get("/ops/x")).await.unwrap();
        assert_eq!(ops.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn debug_does_not_show_the_key() {
        let rule = ApiKeyRule::new(PathMatch::Exact("/log".into()), OPS_KEY);
        let text = format!("{:?}", ApiKeyRules::new(vec![rule]));
        assert!(!text.contains(OPS_KEY));
    }
}
