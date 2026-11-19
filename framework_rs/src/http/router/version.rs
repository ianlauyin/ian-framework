use std::sync::OnceLock;

use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::exception;
use crate::exception::Exception;

pub const VERSION_RANDOM_FAILED: &str = "VERSION_RANDOM_FAILED";

static VERSION: OnceLock<String> = OnceLock::new();

pub fn version_router() -> Router {
    Router::new().route("/version", get(get_version))
}

async fn get_version() -> Result<Json<VersionView>, Exception> {
    let version = match VERSION.get() {
        Some(version) => version.clone(),
        None => {
            let mut bytes = [0u8; 16];
            getrandom::fill(&mut bytes).map_err(|err| {
                exception!(
                    code = VERSION_RANDOM_FAILED,
                    "version random source failed",
                    source = err
                )
            })?;
            VERSION.get_or_init(|| hex::encode(bytes)).clone()
        }
    };
    Ok(Json(VersionView { version }))
}

#[derive(Serialize)]
struct VersionView {
    version: String,
}
