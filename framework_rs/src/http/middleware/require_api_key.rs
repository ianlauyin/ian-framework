use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::exception;
use crate::http::error_code;
use crate::http::headers::bearer::{bearer_token, key_matches};
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
            BTreeMap::from([("method".to_string(), method.as_str().into())]),
        );
        return exception!("unauthorized", code = error_code::UNAUTHORIZED).into_response();
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
