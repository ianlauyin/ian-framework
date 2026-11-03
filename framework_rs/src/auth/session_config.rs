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
            assert_eq!(err.code(), Some("AUTH_CONFIG"));
        }
        let edge = config_with(|config| {
            config.max_age_secs = MAX_AGE_LIMIT_SECS;
            config.ttl_secs = MAX_AGE_LIMIT_SECS;
            config.cookie_name = "__Host-sid".to_string();
        });
        assert!(edge.check().is_ok());
    }
}
