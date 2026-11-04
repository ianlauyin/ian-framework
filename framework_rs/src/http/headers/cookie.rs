use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::HeaderMap;

// HTTP/2 clients can send each cookie in its own Cookie field, so read every field.
pub fn read_cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|text| text.split(';'))
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == name).then_some(value)
        })
}

pub fn sets_cookie(headers: &HeaderMap, name: &str) -> bool {
    headers.get_all(SET_COOKIE).iter().any(|value| {
        value
            .to_str()
            .ok()
            .and_then(|text| text.split_once('='))
            .is_some_and(|(key, _)| key.trim() == name)
    })
}
