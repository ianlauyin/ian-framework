use crate::exception;
use crate::exception::Exception;

pub const SESSION_CONFIG_INVALID: &str = "SESSION_CONFIG_INVALID";

pub const MAX_TTL_SECS: u64 = 400 * 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionConfig {
    pub prefix: String,
    pub cookie_name: String,
    pub ttl_secs: u64,
    pub absolute_ttl_secs: u64,
    pub max_sessions_per_subject: usize,
}

impl SessionConfig {
    pub fn check(&self) -> Result<(), Exception> {
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
        if self.absolute_ttl_secs < self.ttl_secs {
            return Err(exception!(
                code = SESSION_CONFIG_INVALID,
                "session absolute ttl is less than the session ttl"
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
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            prefix: "session:".to_string(),
            cookie_name: "session".to_string(),
            ttl_secs: 30 * 60,
            absolute_ttl_secs: 30 * 24 * 60 * 60,
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
