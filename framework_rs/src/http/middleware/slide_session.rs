use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

use crate::auth::SessionStore;
use crate::http::headers::sets_cookie;

pub async fn slide_session(
    State(store): State<SessionStore>,
    request: Request,
    next: Next,
) -> Response {
    let token = store.token(request.headers()).map(str::to_owned);
    let mut response = next.run(request).await;
    if response.status() == StatusCode::UNAUTHORIZED {
        return response;
    }
    let Some(token) = token else {
        return response;
    };
    if sets_cookie(response.headers(), store.cookie_name()) {
        return response;
    }
    if store.touch(&token).await.unwrap_or(false) {
        if let Ok(value) = HeaderValue::from_str(&store.cookie(&token)) {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::tests::store_or_skip;
    use axum::body::Body;
    use axum::http::Request;
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
        let token = store.create("7").await.unwrap();
        let app = router(store.clone(), StatusCode::OK, false);
        let response = app
            .oneshot(request(Some(&format!("sid={token}"))))
            .await
            .unwrap();
        assert_eq!(set_cookies(&response), vec![store.cookie(&token)]);
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
        let token = store.create("7").await.unwrap();
        let cookie = format!("sid={token}");
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
        let token = store.create("7").await.unwrap();
        let app = router(store, StatusCode::OK, true);
        let response = app
            .oneshot(request(Some(&format!("sid={token}"))))
            .await
            .unwrap();
        assert_eq!(set_cookies(&response), vec!["sid=other".to_string()]);
    }
}
