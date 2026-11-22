use std::collections::BTreeMap;

use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{from_fn, from_fn_with_state};
use axum::routing::post;
use axum::{Json, Router};

use crate::exception;
use crate::exception::Exception;
use crate::http::error_code;
use crate::http::middleware::{RateLimiter, rate_limit, require_same_origin};
use crate::logging::{ClientLog, Logger};

const CLIENT_LOG_PATH: &str = "/log";
pub const BODY_LIMIT: usize = 1024 * 1024;
pub const RATE_CAPACITY: u32 = 300;
pub const RATE_REFILL_PER_SEC: f64 = 10.0;
const RATE_PRUNE_AT: usize = 10_000;

pub fn client_log_router() -> Router {
    let limiter = RateLimiter::new(RATE_CAPACITY, RATE_REFILL_PER_SEC, RATE_PRUNE_AT)
        .expect("the client log rate limit is valid");
    Router::new().route(
        CLIENT_LOG_PATH,
        post(receive)
            .route_layer(from_fn(require_same_origin))
            .route_layer(from_fn_with_state(limiter, rate_limit))
            .layer(DefaultBodyLimit::max(BODY_LIMIT)),
    )
}

async fn receive(headers: HeaderMap, Json(log): Json<ClientLog>) -> Result<StatusCode, Exception> {
    let events = match log.log_events(&headers) {
        Ok(events) => events,
        Err(error) => {
            Logger::warn("CLIENT_LOG/rejected", BTreeMap::new());
            return Err(error);
        }
    };
    if let Err(error) = Logger::direct_send(&events).await {
        return Err(exception!(
            code = error_code::BAD_GATEWAY,
            "log server refused the batch",
            source = error,
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}
