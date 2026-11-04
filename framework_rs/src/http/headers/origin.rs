use axum::http::header::{HOST, ORIGIN};
use axum::http::HeaderMap;

// A browser that does not send Sec-Fetch-Site still sends Origin on a POST.
pub fn is_same_origin(headers: &HeaderMap) -> bool {
    if let Some(site) = headers.get("sec-fetch-site") {
        return site.as_bytes() == b"same-origin";
    }
    let Some(origin) = headers.get(ORIGIN).and_then(|value| value.to_str().ok()) else {
        return false;
    };
    let Some(host) = headers.get(HOST).and_then(|value| value.to_str().ok()) else {
        return false;
    };
    origin
        .split_once("://")
        .is_some_and(|(_, authority)| authority.eq_ignore_ascii_case(host))
}
