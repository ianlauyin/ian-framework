use axum::body::Body;
use axum::extract::Request;

use crate::http::headers::client::*;

#[test]
fn a_header_that_is_not_an_ip_gives_an_empty_ip() {
    let request = Request::builder()
        .header("cf-connecting-ip", "not an ip")
        .header("x-forwarded-for", "203.0.113.8")
        .header("x-real-ip", "203.0.113.5")
        .body(Body::empty())
        .unwrap();
    assert_eq!(client_ip(request.headers()), "");
}
