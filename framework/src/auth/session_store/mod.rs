mod headers;
mod record;
mod setup;

use std::fmt;
use std::sync::Arc;

use redis::aio::ConnectionManager;

use super::{AuthError, AuthErrorKind, SessionConfig};

#[derive(Clone)]
pub struct SessionStore {
    inner: Arc<Inner>,
}

impl SessionStore {
    pub fn ttl_secs(&self) -> u64 {
        self.inner.config.ttl_secs
    }

    fn conn(&self) -> ConnectionManager {
        self.inner.conn.clone()
    }

    fn key(&self, token: &str) -> String {
        format!("{}{token}", self.inner.config.prefix)
    }
}

impl fmt::Debug for SessionStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionStore")
            .field("prefix", &self.inner.config.prefix)
            .field("cookie_name", &self.inner.config.cookie_name)
            .field("ttl_secs", &self.inner.config.ttl_secs)
            .finish_non_exhaustive()
    }
}

struct Inner {
    conn: ConnectionManager,
    config: SessionConfig,
}

fn store_err(err: redis::RedisError) -> AuthError {
    AuthError::with_source(AuthErrorKind::Store, "session store command failed", err)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn store_or_skip() -> Option<SessionStore> {
        let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
        let mut bytes = [0u8; 8];
        getrandom::fill(&mut bytes).unwrap();
        let config = SessionConfig {
            prefix: format!("ian-framework-test:{}:", hex::encode(bytes)),
            cookie_name: "sid".to_string(),
            ttl_secs: 60,
        };
        match SessionStore::connect(&url, config).await {
            Ok(store) => Some(store),
            Err(err) => {
                eprintln!("skipping session store test: {err}");
                None
            }
        }
    }

    #[tokio::test]
    async fn sessions_store_touch_and_remove() {
        let Some(store) = store_or_skip().await else {
            return;
        };
        let first = store.create("7").await.unwrap();
        let second = store.create("7").await.unwrap();
        let other = store.create("8").await.unwrap();
        assert_eq!(first.len(), 64);
        assert_ne!(first, second);
        assert_eq!(store.subject(&first).await.unwrap().as_deref(), Some("7"));
        assert!(store.touch(&first).await.unwrap());

        store.remove(&first).await.unwrap();
        assert_eq!(store.subject(&first).await.unwrap(), None);
        assert!(!store.touch(&first).await.unwrap());

        store.remove_subject("7").await.unwrap();
        assert_eq!(store.subject(&second).await.unwrap(), None);
        assert_eq!(store.subject(&other).await.unwrap().as_deref(), Some("8"));
        store.remove(&other).await.unwrap();
    }

    #[tokio::test]
    async fn debug_shows_only_the_config() {
        let Some(store) = store_or_skip().await else {
            return;
        };
        let text = format!("{store:?}");
        assert!(text.contains("cookie_name: \"sid\""));
        assert!(!text.contains("redis://"));
    }
}
