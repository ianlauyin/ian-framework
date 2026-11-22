use axum;
use tower::ServiceExt;

use ian_framework_rs::auth::{Session, SessionStore};
use ian_framework_rs::http::middleware::require_session::*;

use crate::auth::store_or_skip;

fn router(store: SessionStore, status: axum::http::StatusCode, set_cookie: bool) -> axum::Router {
    axum::Router::new()
        .route(
            "/",
            axum::routing::get(move || async move {
                let mut response = axum::response::IntoResponse::into_response(status);
                if set_cookie {
                    response.headers_mut().append(
                        axum::http::header::SET_COOKIE,
                        axum::http::HeaderValue::from_static("sid=other"),
                    );
                }
                response
            }),
        )
        .layer(axum::middleware::from_fn_with_state(store, require_session))
}

fn request(cookie: Option<&str>) -> axum::extract::Request<axum::body::Body> {
    let mut request = axum::extract::Request::builder().uri("/");
    if let Some(cookie) = cookie {
        request = request.header(axum::http::header::COOKIE, cookie);
    }
    request.body(axum::body::Body::empty()).unwrap()
}

fn set_cookies(response: &axum::response::Response) -> Vec<String> {
    response
        .headers()
        .get_all(axum::http::header::SET_COOKIE)
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
    let app = router(store.clone(), axum::http::StatusCode::OK, false);
    let response = app
        .oneshot(request(Some(&format!("sid={}", session.token))))
        .await
        .unwrap();
    assert_eq!(
        set_cookies(&response),
        vec![store.cookie().set(&session.token)]
    );
}

#[tokio::test]
async fn the_handler_gets_the_loaded_session() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let session = store.create("7").await.unwrap();
    let app = axum::Router::new()
        .route(
            "/",
            axum::routing::get(|session: Session| async move { session.subject }),
        )
        .layer(axum::middleware::from_fn_with_state(
            store.clone(),
            require_session,
        ));
    let response = app
        .clone()
        .oneshot(request(Some(&format!("sid={}", session.token))))
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 64)
        .await
        .unwrap();
    assert_eq!(&body[..], b"7");

    let response = app.oneshot(request(None)).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_missing_or_unknown_session_is_rejected_before_the_handler() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let app = router(store, axum::http::StatusCode::OK, false);
    let response = app.clone().oneshot(request(None)).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
    assert!(set_cookies(&response).is_empty());
    let response = app.oneshot(request(Some("sid=missing"))).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
    assert!(set_cookies(&response).is_empty());
}

#[tokio::test]
async fn no_cookie_is_sent_after_a_401() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let session = store.create("7").await.unwrap();
    let cookie = format!("sid={}", session.token);
    let app = router(store, axum::http::StatusCode::UNAUTHORIZED, false);
    let response = app.oneshot(request(Some(&cookie))).await.unwrap();
    assert!(set_cookies(&response).is_empty());
}

#[tokio::test]
async fn a_cookie_set_by_the_handler_is_kept_alone() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let session = store.create("7").await.unwrap();
    let app = router(store, axum::http::StatusCode::OK, true);
    let response = app
        .oneshot(request(Some(&format!("sid={}", session.token))))
        .await
        .unwrap();
    assert_eq!(set_cookies(&response), vec!["sid=other".to_string()]);
}

#[tokio::test]
async fn the_access_log_gets_the_subject() {
    let Some(store) = store_or_skip().await else {
        return;
    };
    let logger = crate::logging::global_logger();
    let session = store.create("7").await.unwrap();
    let app = router(store.clone(), axum::http::StatusCode::OK, false).layer(
        axum::middleware::from_fn(ian_framework_rs::http::middleware::access_log),
    );
    let request = axum::extract::Request::builder()
        .uri("/?case=subject")
        .header(axum::http::header::COOKIE, format!("sid={}", session.token))
        .body(axum::body::Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let events: Vec<_> = crate::logging::buffered_events(logger)
        .into_iter()
        .filter(|event| {
            event
                .context
                .get("request_url")
                .is_some_and(|url| *url == "/?case=subject")
        })
        .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].context["subject"], "7");
    store.remove_subject("7").await.unwrap();
}
