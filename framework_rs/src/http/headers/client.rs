use std::net::IpAddr;

use axum::http::HeaderMap;

pub fn client_ip(headers: &HeaderMap) -> String {
    header_text(headers, "cf-connecting-ip")
        .and_then(|value| value.parse::<IpAddr>().ok())
        .map(|ip| ip.to_string())
        .unwrap_or_default()
}

pub fn user_agent(headers: &HeaderMap) -> String {
    header_text(headers, "user-agent").unwrap_or_default()
}

pub(crate) fn header_text(headers: &HeaderMap, name: &str) -> Option<String> {
    let value = headers.get(name)?.to_str().ok()?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}
