use std::collections::BTreeMap;
use std::path::Path;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::exception;
use crate::http::error_code::{INTERNAL_ERROR, NOT_FOUND};
use crate::logging::Logger;

const DIST: &str = "frontend/dist";

pub fn spa_router() -> Router {
    Router::new().fallback(spa)
}

pub(crate) fn static_type(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "woff2" => "font/woff2",
        "json" => "application/json",
        "webmanifest" => "application/manifest+json",
        "ico" => "image/x-icon",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

async fn spa(request: Request<Body>) -> Response {
    let dist = Path::new(DIST);
    let root = match dist.canonicalize() {
        Ok(root) => root,
        Err(error) => {
            return internal_error("SPA/distMissing", dist, "spa is missing", error);
        }
    };
    let relative = request.uri().path().trim_start_matches('/');
    let index = root.join("index.html");
    let target = match root.join(relative).canonicalize() {
        Ok(path) if !relative.is_empty() && path.starts_with(&root) && path.is_file() => path,
        _ => index,
    };
    let bytes = match tokio::fs::read(&target).await {
        Ok(bytes) => bytes,
        Err(error) => {
            return internal_error("SPA/fileUnreadable", &target, "spa is missing", error);
        }
    };
    Response::builder()
        .header(header::CONTENT_TYPE, static_type(&target))
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn internal_error(
    action: &str,
    path: &Path,
    message: &'static str,
    error: std::io::Error,
) -> Response {
    let logged = exception!(code = NOT_FOUND, message, source = error);
    Logger::error(
        action,
        BTreeMap::from([("path".to_string(), path.display().to_string().into())]),
        Box::new(logged),
    );
    exception!(code = INTERNAL_ERROR, message).into_response()
}
