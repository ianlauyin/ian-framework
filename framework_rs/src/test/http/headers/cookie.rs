use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::HeaderMap;

use crate::http::headers::cookie::*;
use axum::http::HeaderValue;

#[test]
fn reads_the_first_cookie_with_the_name() {
    let mut headers = HeaderMap::new();
    assert_eq!(read_cookie(&headers, "sid"), None);
    headers.insert(
        COOKIE,
        HeaderValue::from_static("theme=dark; sid=abc; sid=def"),
    );
    assert_eq!(read_cookie(&headers, "sid"), Some("abc"));
    assert_eq!(read_cookie(&headers, "si"), None);
}

#[test]
fn reads_each_cookie_field() {
    let mut headers = HeaderMap::new();
    headers.append(COOKIE, HeaderValue::from_static("theme=dark"));
    headers.append(COOKIE, HeaderValue::from_static("sid=abc"));
    assert_eq!(read_cookie(&headers, "sid"), Some("abc"));
}

#[test]
fn skips_a_non_utf8_field() {
    let mut headers = HeaderMap::new();
    headers.append(COOKIE, HeaderValue::from_bytes(b"sid=\xff").unwrap());
    assert_eq!(read_cookie(&headers, "sid"), None);
    headers.append(COOKIE, HeaderValue::from_static("sid=abc"));
    assert_eq!(read_cookie(&headers, "sid"), Some("abc"));
}

#[test]
fn finds_a_set_cookie_with_the_name() {
    let mut headers = HeaderMap::new();
    assert!(!sets_cookie(&headers, "sid"));
    headers.append(SET_COOKIE, HeaderValue::from_static("theme=dark"));
    headers.append(SET_COOKIE, HeaderValue::from_static("sid_old=x"));
    assert!(!sets_cookie(&headers, "sid"));
    headers.append(SET_COOKIE, HeaderValue::from_static("sid=; Max-Age=0"));
    assert!(sets_cookie(&headers, "sid"));
}
