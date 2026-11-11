use axum::extract::Request;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;

use crate::auth::{Session, SessionStore};

use crate::http::middleware::slide_session::*;
use crate::test::auth::store_or_skip;
use axum::body::Body;
use axum::middleware::from_fn_with_state;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use tower::ServiceExt;

fn router(store: SessionStore, status: StatusCode, set_cookie: bool) -> Router {
    Router::new()
        .route(
            "/",
            get(move || async move {
                let mut response = status.into_response();
                if set_cookie {
                    response
                        .headers_mut()
                        .append(header::SET_COOKIE, HeaderValue::from_static("sid=other"));
                }
                response
            }),
        )
        .layer(from_fn_with_state(store, slide_session))
}

fn request(cookie: Option<&str>) -> Request<Body> {
    let mut request = Request::builder().uri("/");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    request.body(Body::empty()).unwrap()
}

fn set_cookies(response: &Response) -> Vec<String> {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn a_live_session_gets_its_cookie_again() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let session = store.create("7").await.unwrap();
    let app = router(store.clone(), StatusCode::OK, false);
    let response = app
        .oneshot(request(Some(&format!("sid={}", session.token))))
        .await
        .unwrap();
    assert_eq!(set_cookies(&response), vec![store.cookie(&session.token)]);
}

#[tokio::test]
async fn the_handler_gets_the_loaded_session() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let session = store.create("7").await.unwrap();
    let app = Router::new()
        .route("/", get(|session: Session| async move { session.subject }))
        .layer(from_fn_with_state(store.clone(), slide_session));
    let response = app
        .clone()
        .oneshot(request(Some(&format!("sid={}", session.token))))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 64)
        .await
        .unwrap();
    assert_eq!(&body[..], b"7");

    let response = app.oneshot(request(None)).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_unknown_session_gets_no_cookie() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let app = router(store, StatusCode::OK, false);
    let response = app.oneshot(request(Some("sid=missing"))).await.unwrap();
    assert!(set_cookies(&response).is_empty());
}

#[tokio::test]
async fn no_cookie_is_sent_without_a_token_or_after_a_401() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let session = store.create("7").await.unwrap();
    let cookie = format!("sid={}", session.token);
    let app = router(store.clone(), StatusCode::OK, false);
    let response = app.oneshot(request(None)).await.unwrap();
    assert!(set_cookies(&response).is_empty());
    let app = router(store, StatusCode::UNAUTHORIZED, false);
    let response = app.oneshot(request(Some(&cookie))).await.unwrap();
    assert!(set_cookies(&response).is_empty());
}

#[tokio::test]
async fn a_cookie_set_by_the_handler_is_kept_alone() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let session = store.create("7").await.unwrap();
    let app = router(store, StatusCode::OK, true);
    let response = app
        .oneshot(request(Some(&format!("sid={}", session.token))))
        .await
        .unwrap();
    assert_eq!(set_cookies(&response), vec!["sid=other".to_string()]);
}
