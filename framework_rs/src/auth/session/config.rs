use chrono::{DateTime, Utc};

use crate::exception;
use crate::exception::Exception;

// Browsers cap a cookie lifetime at 400 days.
pub(crate) const MAX_AGE_LIMIT_SECS: u64 = 400 * 24 * 60 * 60;

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
        Err(exception!(code = "AUTH_CONFIG", problem))
    }

    pub(crate) fn cookie(&self, token: &str, max_age_secs: u64, now: DateTime<Utc>) -> String {
        let max_age = max_age_secs.min(MAX_AGE_LIMIT_SECS);
        let expires = now + chrono::Duration::seconds(max_age as i64);
        let stamp = expires.format("%a, %d %b %Y %H:%M:%S GMT");
        format!(
            "{}={token}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={max_age}; Expires={stamp}",
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
