use std::any::Any;
use std::collections::BTreeMap;
use std::future::Future;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::exception;
use crate::http::error_code;
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
                    ("method".to_string(), method.as_str().into()),
                    ("path".to_string(), path.into()),
                ]),
                "handler panicked".into(),
            );
            exception!("internal error", code = error_code::INTERNAL_ERROR).into_response()
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
