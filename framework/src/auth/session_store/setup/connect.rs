use std::sync::Arc;
use std::time::Duration;

use redis::aio::{ConnectionManager, ConnectionManagerConfig};

use crate::auth::{AuthError, AuthErrorKind, SessionConfig};

use crate::auth::session_store::{Inner, SessionStore};

impl SessionStore {
    pub async fn connect(url: &str, config: SessionConfig) -> Result<Self, AuthError> {
        if config.ttl_secs == 0 {
            return Err(AuthError::new(AuthErrorKind::Config, "session ttl is zero"));
        }
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

fn unreachable(err: redis::RedisError) -> AuthError {
    AuthError::with_source(AuthErrorKind::Store, "cannot reach redis", err)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
