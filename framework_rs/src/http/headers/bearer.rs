use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::token::is_token68;

pub fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("Bearer") || !is_token68(token) {
        return None;
    }
    Some(token)
}

pub(crate) fn key_matches(token: &str, key: &str) -> bool {
    if key.is_empty() {
        return false;
    }
    let token_hash = Sha256::digest(token.as_bytes());
    let key_hash = Sha256::digest(key.as_bytes());
    bool::from(token_hash.as_slice().ct_eq(key_hash.as_slice()))
}
