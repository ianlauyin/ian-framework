use std::sync::Arc;
use std::time::Duration;

use redis::aio::{ConnectionManager, ConnectionManagerConfig};

use crate::auth::{AuthError, AuthErrorKind, SessionConfig};

use crate::auth::session_store::{Inner, SessionStore};

// Browsers cap a cookie lifetime at 400 days.
const MAX_AGE_LIMIT_SECS: u64 = 400 * 24 * 60 * 60;

impl SessionStore {
    pub async fn connect(url: &str, config: SessionConfig) -> Result<Self, AuthError> {
        check_config(&config)?;
        let client = redis::Client::open(url).map_err(|err| {
            AuthError::with_source(AuthErrorKind::Config, "redis url is not valid", err)
        })?;
        let mut conn = ConnectionManager::new_with_config(client, manager_config())
            .await
            .map_err(unreachable)?;
        let pong: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(unreachable)?;
        if pong != "PONG" {
            return Err(AuthError::new(AuthErrorKind::Store, "cannot reach redis"));
        }
        Ok(Self {
            inner: Arc::new(Inner { conn, config }),
        })
    }
}

// Fail fast when Redis is down instead of retrying for many seconds.
fn manager_config() -> ConnectionManagerConfig {
    ConnectionManagerConfig::new()
        .set_connection_timeout(Some(Duration::from_secs(2)))
        .set_response_timeout(Some(Duration::from_secs(5)))
        .set_number_of_retries(2)
        .set_max_delay(Duration::from_millis(500))
}

fn check_config(config: &SessionConfig) -> Result<(), AuthError> {
    let problem = if config.ttl_secs == 0 {
        "session ttl is zero"
    } else if config.max_age_secs == 0 {
        "session max age is zero"
    } else if config.max_age_secs > MAX_AGE_LIMIT_SECS {
        "session max age is more than 400 days"
    } else if config.ttl_secs > config.max_age_secs {
        "session ttl is more than the max age"
    } else if !is_cookie_name(&config.cookie_name) {
        "session cookie name is not valid"
    } else {
        return Ok(());
    };
    Err(AuthError::new(AuthErrorKind::Config, problem))
}

// RFC 6265 cookie-name: one or more visible ASCII characters with no separator.
fn is_cookie_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && !b"()<>@,;:\\\"/[]?={}".contains(&byte))
}

fn unreachable(err: redis::RedisError) -> AuthError {
    AuthError::with_source(AuthErrorKind::Store, "cannot reach redis", err)
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
        assert!(check_config(&SessionConfig::default()).is_ok());
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
            let err = check_config(&config).unwrap_err();
            assert_eq!(err.kind(), AuthErrorKind::Config);
        }
        let edge = config_with(|config| {
            config.max_age_secs = MAX_AGE_LIMIT_SECS;
            config.ttl_secs = MAX_AGE_LIMIT_SECS;
            config.cookie_name = "__Host-sid".to_string();
        });
        assert!(check_config(&edge).is_ok());
    }

    #[tokio::test]
    async fn bad_config_is_refused_before_connecting() {
        let err = SessionStore::connect("not a redis url", SessionConfig::default())
            .await
            .unwrap_err();
        assert_eq!(err.kind(), AuthErrorKind::Config);
        assert!(!err.to_string().contains("not a redis url"));

        let config = SessionConfig {
            ttl_secs: 0,
            ..SessionConfig::default()
        };
        let err = SessionStore::connect("redis://127.0.0.1:6379", config)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), AuthErrorKind::Config);

        let config = SessionConfig {
            max_age_secs: 0,
            ..SessionConfig::default()
        };
        let err = SessionStore::connect("redis://127.0.0.1:6379", config)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), AuthErrorKind::Config);
    }
}
