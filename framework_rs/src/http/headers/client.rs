use std::net::IpAddr;

use axum::http::HeaderMap;

pub fn client_ip(headers: &HeaderMap) -> Option<&str> {
    header_text(headers, "cf-connecting-ip").filter(|value| value.parse::<IpAddr>().is_ok())
}

pub fn user_agent(headers: &HeaderMap) -> Option<&str> {
    header_text(headers, "user-agent")
}

pub(crate) fn header_text<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let value = headers.get(name)?.to_str().ok()?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}
