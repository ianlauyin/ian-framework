use std::collections::BTreeMap;

use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

use crate::auth::session::LoadFailed;
use crate::auth::SessionStore;
use crate::http::headers::sets_cookie;
use crate::logging::Logger;

pub async fn slide_session(
    State(store): State<SessionStore>,
    mut request: Request,
    next: Next,
) -> Response {
    let loaded = match store.token(request.headers()) {
        Some(token) => store.load(token).await,
        None => Ok(None),
    };
    let session = match loaded {
        Ok(session) => session,
        Err(err) => {
            Logger::error("session_load", BTreeMap::new(), Box::new(err));
            request.extensions_mut().insert(LoadFailed);
            None
        }
    };
    if let Some(session) = &session {
        request.extensions_mut().insert(session.clone());
    }
    let mut response = next.run(request).await;
    let Some(session) = session else {
        return response;
    };
    if response.status() == StatusCode::UNAUTHORIZED
        || sets_cookie(response.headers(), store.cookie_name())
    {
        return response;
    }
    if let Ok(value) = HeaderValue::from_str(&store.cookie(&session)) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    response
}
