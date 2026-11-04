use axum::body::to_bytes;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::from_fn;
use axum::routing::post;
use axum::Router;

use crate::http::middleware::{client_ip, require_same_origin, user_agent};

use super::{LogEvent, LogShipper, SHIP_CHUNK};

const CLIENT_LOG_PATH: &str = "/log";
pub(crate) const BODY_LIMIT: usize = 1024 * 1024;

pub fn client_events(events: &[LogEvent]) -> Option<Vec<LogEvent>> {
    if events.is_empty() || events.len() > SHIP_CHUNK {
        return None;
    }
    Some(events.iter().map(LogEvent::from_client).collect())
}

pub fn client_log_router(shipper: LogShipper) -> Router {
    Router::new()
        .route(
            CLIENT_LOG_PATH,
            post(receive).route_layer(from_fn(require_same_origin)),
        )
        .with_state(shipper)
}

async fn receive(State(shipper): State<LogShipper>, request: Request) -> StatusCode {
    let ip = client_ip(&request);
    let user_agent = user_agent(request.headers());
    let Ok(body) = to_bytes(request.into_body(), BODY_LIMIT).await else {
        return StatusCode::PAYLOAD_TOO_LARGE;
    };
    let Ok(events) = serde_json::from_slice::<Vec<LogEvent>>(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    let Some(events) = client_events(&events) else {
        return StatusCode::BAD_REQUEST;
    };
    for mut event in events {
        event.context.insert("ip".to_string(), ip.clone().into());
        event
            .context
            .insert("user_agent".to_string(), user_agent.clone().into());
        shipper.record(event);
    }
    StatusCode::NO_CONTENT
}
