use axum::http::HeaderMap;

use crate::http::read_cookie;

use crate::auth::session_store::SessionStore;

impl SessionStore {
    pub fn token<'a>(&self, headers: &'a HeaderMap) -> Option<&'a str> {
        read_cookie(headers, &self.inner.config.cookie_name).filter(|value| is_token(value))
    }
}

// A token from `create` is 64 lowercase hex characters. Other values skip the Redis call.
fn is_token(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_the_token_shape() {
        assert!(is_token(&"a1".repeat(32)));
        assert!(!is_token(&"a1".repeat(31)));
        assert!(!is_token(&"A1".repeat(32)));
        assert!(!is_token(&"g1".repeat(32)));
        assert!(!is_token(""));
    }
}
