use std::any::Any;
use std::collections::BTreeMap;
use std::future::Future;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::http::ApiError;
use crate::logging::Logger;

struct CatchUnwind<F>(Pin<Box<F>>);

pub async fn catch_panic(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    match CatchUnwind(Box::pin(next.run(request))).await {
        Ok(response) => response,
        Err(_) => {
            Logger::error(
                "HTTP/handlerPanicked",
                BTreeMap::from([
                    ("method".to_string(), method.as_str().to_string()),
                    ("path".to_string(), path),
                ]),
                "handler panicked".into(),
            );
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "internal error",
            )
            .into_response()
        }
    }
}

impl<F: Future> Future for CatchUnwind<F> {
    type Output = Result<F::Output, Box<dyn Any + Send>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match catch_unwind(AssertUnwindSafe(|| self.0.as_mut().poll(cx))) {
            Ok(Poll::Ready(output)) => Poll::Ready(Ok(output)),
            Ok(Poll::Pending) => Poll::Pending,
            Err(payload) => Poll::Ready(Err(payload)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::middleware::from_fn;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    async fn send(path: &str) -> Response {
        let router = Router::new()
            .route("/ok", get(|| async { "fine" }))
            .route(
                "/panic",
                get(|| async {
                    if true {
                        panic!("boom with a secret");
                    }
                    "unreachable"
                }),
            )
            .route(
                "/late-panic",
                get(|| async {
                    tokio::task::yield_now().await;
                    if true {
                        panic!("late boom");
                    }
                    "unreachable"
                }),
            )
            .layer(from_fn(catch_panic));
        let request = Request::builder().uri(path).body(Body::empty()).unwrap();
        router.oneshot(request).await.unwrap()
    }

    #[tokio::test]
    async fn passes_a_normal_response() {
        let response = send("/ok").await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn turns_a_panic_into_a_500_without_the_detail() {
        for path in ["/panic", "/late-panic"] {
            let response = send(path).await;
            assert_eq!(
                response.status(),
                StatusCode::INTERNAL_SERVER_ERROR,
                "{path}"
            );
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            assert_eq!(
                body, r#"{"code":"internal_error","message":"internal error"}"#,
                "{path}"
            );
        }
    }
}
