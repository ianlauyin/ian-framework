use chrono::{DateTime, Duration, Utc};

use crate::auth::session_store::SessionStore;
use crate::auth::SessionConfig;

impl SessionStore {
    pub fn cookie(&self, token: &str) -> String {
        cookie_value(&self.inner.config, token, Utc::now())
    }
}

fn cookie_value(config: &SessionConfig, token: &str, now: DateTime<Utc>) -> String {
    let ttl = config.ttl_secs;
    let expires = now + Duration::seconds(i64::try_from(ttl).unwrap_or(i64::MAX / 1000));
    let stamp = expires.format("%a, %d %b %Y %H:%M:%S GMT");
    format!(
        "{}={token}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={ttl}; Expires={stamp}",
        config.cookie_name
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookie_carries_max_age_and_expires() {
        let config = SessionConfig {
            prefix: "p:".to_string(),
            cookie_name: "sid".to_string(),
            ttl_secs: 3600,
        };
        let now = DateTime::parse_from_rfc3339("2026-10-24T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            cookie_value(&config, "abc", now),
            "sid=abc; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=3600; Expires=Sat, 24 Oct 2026 11:00:00 GMT"
        );
    }
}
