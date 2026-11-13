use std::collections::BTreeMap;

use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::from_fn;
use axum::routing::post;
use axum::{Json, Router};

use crate::http::middleware::require_same_origin;
use crate::logging::{ClientLog, Logger};

const CLIENT_LOG_PATH: &str = "/log";
pub(crate) const BODY_LIMIT: usize = 1024 * 1024;

pub fn client_log_router() -> Router {
    Router::new().route(
        CLIENT_LOG_PATH,
        post(receive)
            .route_layer(from_fn(require_same_origin))
            .layer(DefaultBodyLimit::max(BODY_LIMIT)),
    )
}

async fn receive(headers: HeaderMap, Json(log): Json<ClientLog>) -> StatusCode {
    let Some(events) = log.log_events(&headers) else {
        Logger::warn("CLIENT_LOG/rejected", BTreeMap::new());
        return StatusCode::BAD_REQUEST;
    };
    if Logger::direct_send(&events).await {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::BAD_GATEWAY
    }
}
