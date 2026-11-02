use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::auth::SessionStore;
use crate::http::ApiError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSubject(pub String);

pub async fn require_session(
    State(store): State<SessionStore>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(token) = store.token(request.headers()).map(str::to_owned) else {
        return not_signed_in();
    };
    match store.subject(&token).await {
        Ok(Some(subject)) => {
            request.extensions_mut().insert(SessionSubject(subject));
            next.run(request).await
        }
        Ok(None) => not_signed_in(),
        Err(error) => ApiError::from(error).into_response(),
    }
}

fn not_signed_in() -> Response {
    ApiError::new(StatusCode::UNAUTHORIZED, "not_signed_in", "not signed in").into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::SessionConfig;
    use axum::body::{to_bytes, Body};
    use axum::middleware::from_fn_with_state;
    use axum::routing::get;
    use axum::{Extension, Router};
    use tower::ServiceExt;

    async fn who(Extension(SessionSubject(subject)): Extension<SessionSubject>) -> String {
        subject
    }

    async fn store_or_skip() -> Option<SessionStore> {
        let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
        let config = SessionConfig {
            prefix: "my-app:test-session:".to_string(),
            ..SessionConfig::default()
        };
        match SessionStore::connect(&url, config).await {
            Ok(store) => Some(store),
            Err(error) => {
                eprintln!("skipping require_session tests: {error}");
                None
            }
        }
    }

    async fn send(router: &Router, cookie: Option<String>) -> Response {
        let mut request = Request::builder().uri("/");
        if let Some(cookie) = cookie {
            request = request.header("cookie", cookie);
        }
        router
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn passes_a_signed_in_request_with_the_subject() {
        let Some(store) = store_or_skip().await else {
            return;
        };
        let router = Router::new()
            .route("/", get(who))
            .layer(from_fn_with_state(store.clone(), require_session));
        let token = store.create("user-1").await.unwrap();
        let cookie = format!("{}={token}", store.cookie_name());
        let response = send(&router, Some(cookie)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body, "user-1");
        store.remove(&token).await.unwrap();
    }

    #[tokio::test]
    async fn rejects_a_missing_or_unknown_session_with_401() {
        let Some(store) = store_or_skip().await else {
            return;
        };
        let router = Router::new()
            .route("/", get(who))
            .layer(from_fn_with_state(store.clone(), require_session));
        let unknown = format!("{}=not-a-token", store.cookie_name());
        for cookie in [None, Some(unknown)] {
            let response = send(&router, cookie).await;
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            assert_eq!(
                body,
                r#"{"code":"not_signed_in","message":"not signed in"}"#
            );
        }
    }
}
