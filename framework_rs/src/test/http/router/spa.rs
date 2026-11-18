use std::fs;
use std::path::{Path, PathBuf};

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use tokio::sync::Mutex;
use tower::ServiceExt;

use crate::http::router::spa::*;

static WORKING_DIR: Mutex<()> = Mutex::const_new(());

fn make_root(name: &str, with_dist: bool) -> PathBuf {
    let root = std::env::temp_dir().join(format!("my-app-spa-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    if with_dist {
        let dist = root.join("frontend/dist");
        fs::create_dir_all(dist.join("assets")).unwrap();
        fs::write(dist.join("index.html"), "index").unwrap();
        fs::write(dist.join("assets/app.js"), "script").unwrap();
    }
    root
}

async fn get(root: &Path, uri: &str) -> (StatusCode, String, String) {
    let _guard = WORKING_DIR.lock().await;
    std::env::set_current_dir(root).unwrap();
    let response = spa_router()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .map(|value| value.to_str().unwrap().to_string())
        .unwrap_or_default();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        content_type,
        String::from_utf8(body.to_vec()).unwrap(),
    )
}

#[tokio::test]
async fn existing_file_is_served_with_its_type() {
    let root = make_root("file", true);
    let (status, content_type, body) = get(&root, "/assets/app.js").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "text/javascript; charset=utf-8");
    assert_eq!(body, "script");
}

#[tokio::test]
async fn root_serves_index() {
    let root = make_root("root", true);
    let (status, content_type, body) = get(&root, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "text/html; charset=utf-8");
    assert_eq!(body, "index");
}

#[tokio::test]
async fn missing_file_serves_index() {
    let root = make_root("missing", true);
    let (status, _, body) = get(&root, "/some/route").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "index");
}

#[tokio::test]
async fn path_outside_dist_serves_index() {
    let root = make_root("outside", true);
    fs::write(root.join("secret.txt"), "secret").unwrap();
    let (status, _, body) = get(&root, "/../../secret.txt").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "index");
}

#[tokio::test]
async fn missing_dist_directory_is_internal_error() {
    let root = make_root("no-dist", false);
    let (status, _, body) = get(&root, "/").await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!body.contains("no-dist"));
}

#[tokio::test]
async fn unreadable_index_is_internal_error() {
    let root = make_root("no-index", true);
    fs::remove_file(root.join("frontend/dist/index.html")).unwrap();
    let (status, _, _) = get(&root, "/some/route").await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn unknown_extension_is_octet_stream() {
    assert_eq!(static_type(Path::new("a.bin")), "application/octet-stream");
    assert_eq!(static_type(Path::new("a")), "application/octet-stream");
}
