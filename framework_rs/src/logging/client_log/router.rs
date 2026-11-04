use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::from_fn;
use axum::routing::post;
use axum::{Json, Router};

use crate::http::middleware::require_same_origin;
use crate::logging::{ClientLog, LogShipper};

const CLIENT_LOG_PATH: &str = "/log";
pub(crate) const BODY_LIMIT: usize = 1024 * 1024;

pub fn client_log_router(shipper: LogShipper) -> Router {
    Router::new()
        .route(
            CLIENT_LOG_PATH,
            post(receive)
                .route_layer(from_fn(require_same_origin))
                .layer(DefaultBodyLimit::max(BODY_LIMIT)),
        )
        .with_state(shipper)
}

async fn receive(
    State(shipper): State<LogShipper>,
    headers: HeaderMap,
    Json(log): Json<ClientLog>,
) -> StatusCode {
    let Some(events) = log.log_events(&headers) else {
        return StatusCode::BAD_REQUEST;
    };
    if shipper.send(events).await {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::BAD_GATEWAY
    }
}
