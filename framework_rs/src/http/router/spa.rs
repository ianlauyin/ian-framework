use std::path::{Path, PathBuf};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::exception;
use crate::exception::Severity;
use crate::http::error_code::{INTERNAL_ERROR, NOT_FOUND};
use crate::logging::LogEvent;
use crate::logging::record;

const DIST: &str = "frontend/dist";

pub fn spa_router() -> Router {
    Router::new().fallback(spa)
}

pub fn static_type(path: &Path) -> &'static str {
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
    let root = match tokio::fs::canonicalize(dist).await {
        Ok(root) => root,
        Err(error) => {
            return internal_error("SPA/distMissing", dist, "spa is missing", error);
        }
    };
    let relative = request.uri().path().trim_start_matches('/');
    let target = match find_file(&root, relative).await {
        Some(path) => path,
        None if is_file_name(relative) => {
            return exception!(
                severity = Severity::Warn,
                code = NOT_FOUND,
                "file not found"
            )
            .into_response();
        }
        None => root.join("index.html"),
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

async fn find_file(root: &Path, relative: &str) -> Option<PathBuf> {
    if relative.is_empty() {
        return None;
    }
    let path = tokio::fs::canonicalize(root.join(relative)).await.ok()?;
    if !path.starts_with(root) || !tokio::fs::metadata(&path).await.ok()?.is_file() {
        return None;
    }
    Some(path)
}

fn is_file_name(relative: &str) -> bool {
    relative
        .rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.') && !name.starts_with('.'))
}

fn internal_error(
    action: &str,
    path: &Path,
    message: &'static str,
    error: std::io::Error,
) -> Response {
    let logged = exception!(code = INTERNAL_ERROR, message, source = error);
    record(LogEvent::error_from(action, &logged).with_context("path", path.display().to_string()));
    exception!(code = INTERNAL_ERROR, message).into_response()
}
