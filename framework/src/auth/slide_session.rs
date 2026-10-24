use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

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

fn sets_cookie(headers: &HeaderMap, name: &str) -> bool {
    headers.get_all(header::SET_COOKIE).iter().any(|value| {
        value
            .to_str()
            .ok()
            .and_then(|text| text.split_once('='))
            .is_some_and(|(key, _)| key.trim() == name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_set_cookie_with_the_name() {
        let mut headers = HeaderMap::new();
        assert!(!sets_cookie(&headers, "sid"));
        headers.append(header::SET_COOKIE, HeaderValue::from_static("theme=dark"));
        headers.append(header::SET_COOKIE, HeaderValue::from_static("sid_old=x"));
        assert!(!sets_cookie(&headers, "sid"));
        headers.append(
            header::SET_COOKIE,
            HeaderValue::from_static("sid=; Max-Age=0"),
        );
        assert!(sets_cookie(&headers, "sid"));
    }
}
