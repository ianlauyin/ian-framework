use crate::exception;
use crate::exception::Exception;

pub const SESSION_CONFIG_INVALID: &str = "SESSION_CONFIG_INVALID";

pub(crate) const MAX_TTL_SECS: u64 = 400 * 24 * 60 * 60;

const COOKIE_ATTRIBUTES: &str = "HttpOnly; Secure; SameSite=Lax; Path=/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionConfig {
    pub prefix: String,
    pub cookie_name: String,
    pub ttl_secs: u64,
    pub max_sessions_per_subject: usize,
}

impl SessionConfig {
    pub(crate) fn check(&self) -> Result<(), Exception> {
        if self.ttl_secs == 0 {
            return Err(exception!(
                code = SESSION_CONFIG_INVALID,
                "session ttl is zero"
            ));
        }
        if self.ttl_secs > MAX_TTL_SECS {
            return Err(exception!(
                code = SESSION_CONFIG_INVALID,
                "session ttl is more than 400 days"
            ));
        }
        if self.max_sessions_per_subject == 0 {
            return Err(exception!(
                code = SESSION_CONFIG_INVALID,
                "session max sessions per subject is zero"
            ));
        }
        if !is_cookie_name(&self.cookie_name) {
            return Err(exception!(
                code = SESSION_CONFIG_INVALID,
                "session cookie name is not valid"
            ));
        }
        Ok(())
    }

    pub(crate) fn cookie(&self, token: &str) -> String {
        format!(
            "{}={token}; {COOKIE_ATTRIBUTES}; Max-Age={}",
            self.cookie_name, self.ttl_secs
        )
    }

    pub(crate) fn clear_cookie(&self) -> String {
        format!("{}=; {COOKIE_ATTRIBUTES}; Max-Age=0", self.cookie_name)
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            prefix: "session:".to_string(),
            cookie_name: "session".to_string(),
            ttl_secs: 7 * 24 * 60 * 60,
            max_sessions_per_subject: 100,
        }
    }
}

fn is_cookie_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && !b"()<>@,;:\\\"/[]?={}".contains(&byte))
}
