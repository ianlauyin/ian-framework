use chrono::{DateTime, Utc};

use crate::auth::SessionConfig;
use crate::exception;
use crate::exception::Exception;

pub(crate) fn random_token() -> Result<String, Exception> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|err| {
        exception!(
            format!("os random source failed: {err}"),
            code = "AUTH_RANDOM"
        )
    })?;
    Ok(hex::encode(bytes))
}

// A token from `SessionStore::create` is 64 lowercase hex characters. Other values skip the Redis call.
pub(crate) fn is_token(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn cookie_value(config: &SessionConfig, token: &str, now: DateTime<Utc>) -> String {
    let ttl = config.ttl_secs;
    let expires = now + chrono::Duration::seconds(i64::try_from(ttl).unwrap_or(i64::MAX / 1000));
    let stamp = expires.format("%a, %d %b %Y %H:%M:%S GMT");
    format!(
        "{}={token}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={ttl}; Expires={stamp}",
        config.cookie_name
    )
}

pub(crate) fn clear_value(cookie_name: &str) -> String {
    format!(
        "{cookie_name}=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_64_lowercase_hex_characters() {
        let token = random_token().unwrap();
        assert_eq!(token.len(), 64);
        assert!(token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
        assert_ne!(token, random_token().unwrap());
    }

    #[test]
    fn accepts_only_the_token_shape() {
        assert!(is_token(&"a1".repeat(32)));
        assert!(!is_token(&"a1".repeat(31)));
        assert!(!is_token(&"A1".repeat(32)));
        assert!(!is_token(&"g1".repeat(32)));
        assert!(!is_token(""));
    }

    #[test]
    fn cookie_carries_max_age_and_expires() {
        let config = SessionConfig {
            prefix: "p:".to_string(),
            cookie_name: "sid".to_string(),
            ttl_secs: 3600,
            max_age_secs: 7200,
        };
        let now = DateTime::parse_from_rfc3339("2026-10-24T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            cookie_value(&config, "abc", now),
            "sid=abc; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=3600; Expires=Sat, 24 Oct 2026 11:00:00 GMT"
        );
    }

    #[test]
    fn clear_cookie_expires_at_once() {
        assert_eq!(
            clear_value("my_session"),
            "my_session=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT"
        );
    }
}
