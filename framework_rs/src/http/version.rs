use std::sync::OnceLock;

use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

static VERSION: OnceLock<String> = OnceLock::new();

pub fn version_router() -> Router {
    Router::new().route("/version", get(get_version))
}

async fn get_version() -> Result<Json<VersionView>, StatusCode> {
    if let Some(version) = VERSION.get() {
        return Ok(Json(VersionView {
            version: version.clone(),
        }));
    }
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let version = VERSION.get_or_init(|| hex::encode(bytes));
    Ok(Json(VersionView {
        version: version.clone(),
    }))
}

#[derive(Serialize)]
struct VersionView {
    version: String,
}
