use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

use crate::http::headers::sets_cookie;

use super::SessionStore;

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
