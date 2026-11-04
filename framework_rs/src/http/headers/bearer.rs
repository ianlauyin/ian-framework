use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("Bearer") || !is_token68(token) {
        return None;
    }
    Some(token)
}

// Slices of different lengths compare in less time, so compare the fixed-size hashes.
pub fn key_matches(token: &str, key: &str) -> bool {
    if key.is_empty() {
        return false;
    }
    let token_hash = Sha256::digest(token.as_bytes());
    let key_hash = Sha256::digest(key.as_bytes());
    bool::from(token_hash.as_slice().ct_eq(key_hash.as_slice()))
}

// RFC 9110 token68: one or more of ALPHA DIGIT "-" "." "_" "~" "+" "/", then any "=".
pub fn is_token68(token: &str) -> bool {
    let body = token.trim_end_matches('=');
    !body.is_empty()
        && body
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._~+/".contains(&byte))
}
