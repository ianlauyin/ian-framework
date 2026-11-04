use axum::http::HeaderMap;

use crate::http::headers::origin::*;
use axum::http::HeaderValue;

fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (name, value) in pairs {
        headers.insert(*name, HeaderValue::from_str(value).unwrap());
    }
    headers
}

#[test]
fn uses_sec_fetch_site_when_present() {
    assert!(is_same_origin(&headers(&[(
        "sec-fetch-site",
        "same-origin"
    )])));
    for site in ["same-site", "cross-site", "none"] {
        assert!(!is_same_origin(&headers(&[("sec-fetch-site", site)])));
    }
    let cross = headers(&[
        ("sec-fetch-site", "cross-site"),
        ("origin", "https://app.example"),
        ("host", "app.example"),
    ]);
    assert!(!is_same_origin(&cross));
}

#[test]
fn falls_back_to_origin_and_host() {
    let same = headers(&[
        ("origin", "https://App.example:8443"),
        ("host", "app.example:8443"),
    ]);
    assert!(is_same_origin(&same));
    let other = headers(&[("origin", "https://evil.example"), ("host", "app.example")]);
    assert!(!is_same_origin(&other));
    let null = headers(&[("origin", "null"), ("host", "app.example")]);
    assert!(!is_same_origin(&null));
    assert!(!is_same_origin(&headers(&[(
        "origin",
        "https://app.example"
    )])));
    assert!(!is_same_origin(&HeaderMap::new()));
}
