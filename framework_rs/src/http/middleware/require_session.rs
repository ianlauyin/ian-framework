use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::auth::SessionStore;
use crate::exception;
use crate::http::error_code;
use crate::http::headers::sets_cookie;

pub async fn require_session(
    State(store): State<SessionStore>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(token) = store.token(request.headers()) else {
        return unauthorized();
    };
    let session = match store.load(token).await {
        Ok(Some(session)) => session,
        Ok(None) => return unauthorized(),
        Err(err) => return err.into_response(),
    };
    request.extensions_mut().insert(session.clone());
    let mut response = next.run(request).await;
    let keep = response.status() == StatusCode::UNAUTHORIZED
        || sets_cookie(response.headers(), store.cookie_name());
    if !keep {
        if let Ok(value) = HeaderValue::from_str(&store.cookie(&session.token)) {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
    response
}

fn unauthorized() -> Response {
    exception!(code = error_code::UNAUTHORIZED, "unauthorized").into_response()
}
