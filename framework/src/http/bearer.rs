use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;
use subtle::ConstantTimeEq;

pub fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    if scheme != "Bearer" || token.is_empty() || token.contains(' ') {
        return None;
    }
    Some(token)
}

pub fn key_matches(token: &str, key: &str) -> bool {
    bool::from(token.as_bytes().ct_eq(key.as_bytes()))
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
    fn rejects_bad_credentials() {
        assert!(!authorize(&HeaderMap::new()));
        assert!(!authorize(&headers_with("bearer test-api-key-value")));
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
        assert_eq!(bearer_token(&headers_with("bearer token")), None);
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
        assert!(key_matches("", ""));
    }
}
