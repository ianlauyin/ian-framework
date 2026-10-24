mod cookies;
mod record;
mod setup;

use std::fmt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use redis::aio::ConnectionManager;
use sha2::{Digest, Sha256};

use super::{AuthError, AuthErrorKind, SessionConfig};

#[derive(Clone)]
pub struct SessionStore {
    inner: Arc<Inner>,
}

impl SessionStore {
    pub fn ttl_secs(&self) -> u64 {
        self.inner.config.ttl_secs
    }

    pub(crate) fn cookie_name(&self) -> &str {
        &self.inner.config.cookie_name
    }

    fn conn(&self) -> ConnectionManager {
        self.inner.conn.clone()
    }

    // A copy of Redis must not give working tokens, so the key holds only the token hash.
    fn key(&self, token: &str) -> String {
        let hash = hex::encode(Sha256::digest(token.as_bytes()));
        format!("{}{hash}", self.inner.config.prefix)
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

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}

// The stored value is `{deadline}:{subject}`. The deadline is the end of the session in Unix seconds.
fn parse_value(value: &str) -> Option<(u64, &str)> {
    let (deadline, subject) = value.split_once(':')?;
    Some((deadline.parse().ok()?, subject))
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
            max_age_secs: 120,
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
    async fn redis_holds_the_token_hash_and_the_deadline() {
        let Some(store) = store_or_skip().await else {
            return;
        };
        let token = store.create("7").await.unwrap();
        let mut conn = store.conn();
        let raw: Option<String> = redis::cmd("GET")
            .arg(format!("{}{token}", store.inner.config.prefix))
            .query_async(&mut conn)
            .await
            .unwrap();
        assert_eq!(raw, None);
        let value: String = redis::cmd("GET")
            .arg(store.key(&token))
            .query_async(&mut conn)
            .await
            .unwrap();
        let (deadline, subject) = parse_value(&value).unwrap();
        assert_eq!(subject, "7");
        assert!(deadline > now_secs() + 110 && deadline <= now_secs() + 120);
        store.remove(&token).await.unwrap();
    }

    #[tokio::test]
    async fn touch_stops_at_the_deadline() {
        let Some(store) = store_or_skip().await else {
            return;
        };
        let mut conn = store.conn();
        let token = "a".repeat(64);
        let key = store.key(&token);
        let near = format!("{}:7", now_secs() + 10);
        redis::cmd("SET")
            .arg(&key)
            .arg(&near)
            .arg("EX")
            .arg(10)
            .query_async::<()>(&mut conn)
            .await
            .unwrap();
        assert!(store.touch(&token).await.unwrap());
        let ttl: i64 = redis::cmd("TTL")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .unwrap();
        assert!(ttl <= 10);

        let past = format!("{}:7", now_secs() - 1);
        redis::cmd("SET")
            .arg(&key)
            .arg(&past)
            .arg("EX")
            .arg(10)
            .query_async::<()>(&mut conn)
            .await
            .unwrap();
        assert_eq!(store.subject(&token).await.unwrap(), None);
        assert!(!store.touch(&token).await.unwrap());
        store.remove(&token).await.unwrap();
    }

    #[test]
    fn value_splits_at_the_first_colon() {
        assert_eq!(parse_value("100:user:7"), Some((100, "user:7")));
        assert_eq!(parse_value("7"), None);
        assert_eq!(parse_value("x:7"), None);
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
