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
    let response = next.run(request).await;
    refresh_session(store, token, response).await
}

async fn refresh_session(
    store: SessionStore,
    token: Option<String>,
    response: Response,
) -> Response {
    if response.status() == StatusCode::UNAUTHORIZED {
        return response;
    }
    add_session_cookie(store, token, response).await
}

async fn add_session_cookie(
    store: SessionStore,
    token: Option<String>,
    response: Response,
) -> Response {
    let Some(token) = token else {
        return response;
    };
    write_session_cookie(store, token, response).await
}

async fn write_session_cookie(store: SessionStore, token: String, response: Response) -> Response {
    if sets_cookie(response.headers(), store.cookie_name()) {
        return response;
    }
    touch_cookie(store, token, response).await
}

async fn touch_cookie(store: SessionStore, token: String, response: Response) -> Response {
    if store.touch(&token).await.unwrap_or(false) {
        return with_cookie(store, &token, response);
    }
    response
}

fn with_cookie(store: SessionStore, token: &str, mut response: Response) -> Response {
    if let Ok(value) = HeaderValue::from_str(&store.cookie(token)) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    response
}
