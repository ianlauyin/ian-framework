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
fn is_token68(token: &str) -> bool {
    let body = token.trim_end_matches('=');
    !body.is_empty()
        && body
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._~+/".contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    const KEY: &str = "test-api-key-value";

    fn headers_with(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_str(value).unwrap());
        headers
    }

    fn authorize(headers: &HeaderMap) -> bool {
        bearer_token(headers).is_some_and(|token| key_matches(token, KEY))
    }

    #[test]
    fn accepts_the_bearer_key() {
        let headers = headers_with("Bearer test-api-key-value");
        assert_eq!(bearer_token(&headers), Some(KEY));
        assert!(authorize(&headers));
    }

    #[test]
    fn accepts_the_scheme_in_any_case() {
        assert!(authorize(&headers_with("bearer test-api-key-value")));
        assert!(authorize(&headers_with("BEARER test-api-key-value")));
    }

    #[test]
    fn rejects_bad_credentials() {
        assert!(!authorize(&HeaderMap::new()));
        assert!(!authorize(&headers_with("Basic test-api-key-value")));
        assert!(!authorize(&headers_with("Bearer")));
        assert!(!authorize(&headers_with("Bearer test-api-key-valu")));
        assert!(!authorize(&headers_with("Bearer test-api-key-value ")));
        assert!(!authorize(&headers_with("Bearer other-key-value!!")));
    }

    #[test]
    fn rejects_a_malformed_header() {
        assert_eq!(bearer_token(&HeaderMap::new()), None);
        assert_eq!(bearer_token(&headers_with("Bearer")), None);
        assert_eq!(bearer_token(&headers_with("Bearer ")), None);
        assert_eq!(bearer_token(&headers_with("Bearer  token")), None);
        assert_eq!(bearer_token(&headers_with("Bearer a b")), None);
        assert_eq!(bearer_token(&headers_with("Bearer a\tb")), None);
        assert_eq!(bearer_token(&headers_with("Bearer ===")), None);
        assert_eq!(bearer_token(&headers_with("Bearer a=b")), None);
    }

    #[test]
    fn accepts_token68_characters() {
        let token = "aZ09-._~+/==";
        assert_eq!(
            bearer_token(&headers_with(&format!("Bearer {token}"))),
            Some(token)
        );
    }

    #[test]
    fn rejects_a_non_utf8_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_bytes(&[0xff, 0xfe]).unwrap(),
        );
        assert_eq!(bearer_token(&headers), None);
        assert!(!authorize(&headers));
    }

    #[test]
    fn compares_the_key_bytes() {
        assert!(key_matches(KEY, KEY));
        assert!(!key_matches("test-api-key-valu", KEY));
        assert!(!key_matches("", KEY));
    }

    #[test]
    fn an_empty_key_never_matches() {
        assert!(!key_matches("", ""));
        assert!(!key_matches("anything", ""));
    }
}
