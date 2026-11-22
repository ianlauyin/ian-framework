use axum::http::HeaderMap;
use axum::http::header::COOKIE;

use ian_framework_rs::auth::session::cookie::*;

fn headers(cookie: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(COOKIE, cookie.parse().unwrap());
    headers
}

#[test]
fn set_and_clear_share_the_attributes() {
    let cookie = SessionCookie::new("sid", 3600);
    assert_eq!(cookie.name(), "sid");
    assert_eq!(
        cookie.set("abc"),
        "sid=abc; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=3600"
    );
    assert_eq!(
        cookie.clear(),
        "sid=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0"
    );
}

#[test]
fn read_gives_only_a_token_of_the_named_cookie() {
    let cookie = SessionCookie::new("sid", 3600);
    let token = "a".repeat(64);
    assert_eq!(
        cookie.read(&headers(&format!("theme=dark; sid={token}"))),
        Some(token.as_str())
    );
    assert_eq!(cookie.read(&headers(&format!("other={token}"))), None);
    assert_eq!(cookie.read(&headers("sid=missing")), None);
    assert_eq!(cookie.read(&HeaderMap::new()), None);
}
