use chrono::{DateTime, Utc};

use crate::exception;
use crate::exception::Exception;

// Browsers cap a cookie lifetime at 400 days.
const MAX_AGE_LIMIT_SECS: u64 = 400 * 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionConfig {
    pub prefix: String,
    pub cookie_name: String,
    pub ttl_secs: u64,
    pub max_age_secs: u64,
}

impl SessionConfig {
    pub(crate) fn check(&self) -> Result<(), Exception> {
        let problem = if self.ttl_secs == 0 {
            "session ttl is zero"
        } else if self.max_age_secs == 0 {
            "session max age is zero"
        } else if self.max_age_secs > MAX_AGE_LIMIT_SECS {
            "session max age is more than 400 days"
        } else if self.ttl_secs > self.max_age_secs {
            "session ttl is more than the max age"
        } else if !is_cookie_name(&self.cookie_name) {
            "session cookie name is not valid"
        } else {
            return Ok(());
        };
        Err(exception!(problem, code = "AUTH_CONFIG"))
    }

    pub(crate) fn cookie(&self, token: &str, now: DateTime<Utc>) -> String {
        let ttl = self.ttl_secs;
        let expires =
            now + chrono::Duration::seconds(i64::try_from(ttl).unwrap_or(i64::MAX / 1000));
        let stamp = expires.format("%a, %d %b %Y %H:%M:%S GMT");
        format!(
            "{}={token}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={ttl}; Expires={stamp}",
            self.cookie_name
        )
    }

    pub(crate) fn clear_cookie(&self) -> String {
        format!(
            "{}=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT",
            self.cookie_name
        )
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            prefix: "session:".to_string(),
            cookie_name: "session".to_string(),
            ttl_secs: 7 * 24 * 60 * 60,
            max_age_secs: 30 * 24 * 60 * 60,
        }
    }
}

// RFC 6265 cookie-name: one or more visible ASCII characters with no separator.
fn is_cookie_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && !b"()<>@,;:\\\"/[]?={}".contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with(change: impl FnOnce(&mut SessionConfig)) -> SessionConfig {
        let mut config = SessionConfig::default();
        change(&mut config);
        config
    }

    #[test]
    fn checks_the_config() {
        assert!(SessionConfig::default().check().is_ok());
        let bad = [
            config_with(|config| config.ttl_secs = 0),
            config_with(|config| config.max_age_secs = 0),
            config_with(|config| config.max_age_secs = MAX_AGE_LIMIT_SECS + 1),
            config_with(|config| config.ttl_secs = config.max_age_secs + 1),
            config_with(|config| config.ttl_secs = u64::MAX),
            config_with(|config| config.cookie_name = String::new()),
            config_with(|config| config.cookie_name = "a b".to_string()),
            config_with(|config| config.cookie_name = "a;b".to_string()),
            config_with(|config| config.cookie_name = "a=b".to_string()),
            config_with(|config| config.cookie_name = "séance".to_string()),
        ];
        for config in bad {
            let err = config.check().unwrap_err();
            assert_eq!(err.code(), "AUTH_CONFIG");
        }
        let edge = config_with(|config| {
            config.max_age_secs = MAX_AGE_LIMIT_SECS;
            config.ttl_secs = MAX_AGE_LIMIT_SECS;
            config.cookie_name = "__Host-sid".to_string();
        });
        assert!(edge.check().is_ok());
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
            config.cookie("abc", now),
            "sid=abc; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=3600; Expires=Sat, 24 Oct 2026 11:00:00 GMT"
        );
    }

    #[test]
    fn clear_cookie_expires_at_once() {
        let config = config_with(|config| config.cookie_name = "my_session".to_string());
        assert_eq!(
            config.clear_cookie(),
            "my_session=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT"
        );
    }
}
