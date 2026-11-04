use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

pub fn health_router() -> Router {
    Router::new().route("/health", get(get_health))
}

async fn get_health() -> Json<HealthView> {
    Json(HealthView { ok: true })
}

#[derive(Serialize)]
struct HealthView {
    ok: bool,
}
