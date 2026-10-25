use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::http::HeaderMap;
use chrono::{DateTime, Utc};
use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use redis::AsyncCommands;
use sha2::{Digest, Sha256};

use crate::http::read_cookie;

use super::{AuthError, AuthErrorKind, SessionConfig};

// Browsers cap a cookie lifetime at 400 days.
const MAX_AGE_LIMIT_SECS: u64 = 400 * 24 * 60 * 60;

#[derive(Clone)]
pub struct SessionStore {
    inner: Arc<Inner>,
}

impl SessionStore {
    pub fn ttl_secs(&self) -> u64 {
        self.inner.config.ttl_secs
    }

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

    pub async fn create(&self, subject: &str) -> Result<String, AuthError> {
        let token = random_token()?;
        let max_age = self.inner.config.max_age_secs;
        let deadline = now_secs().saturating_add(max_age);
        redis::cmd("SET")
            .arg(self.key(&token))
            .arg(format!("{deadline}:{subject}"))
            .arg("EX")
            .arg(self.ttl_secs().min(max_age))
            .query_async::<()>(&mut self.conn())
            .await
            .map_err(store_err)?;
        Ok(token)
    }

    pub async fn subject(&self, token: &str) -> Result<Option<String>, AuthError> {
        let value: Option<String> = redis::cmd("GET")
            .arg(self.key(token))
            .query_async(&mut self.conn())
            .await
            .map_err(store_err)?;
        let subject = value
            .as_deref()
            .and_then(parse_value)
            .and_then(|(deadline, subject)| (deadline > now_secs()).then(|| subject.to_string()));
        Ok(subject)
    }

    pub async fn touch(&self, token: &str) -> Result<bool, AuthError> {
        let key = self.key(token);
        let mut conn = self.conn();
        let value: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(store_err)?;
        let Some((deadline, _)) = value.as_deref().and_then(parse_value) else {
            return Ok(false);
        };
        let left = deadline.saturating_sub(now_secs());
        if left == 0 {
            return Ok(false);
        }
        redis::cmd("EXPIRE")
            .arg(&key)
            .arg(self.ttl_secs().min(left))
            .query_async(&mut conn)
            .await
            .map_err(store_err)
    }

    pub async fn remove(&self, token: &str) -> Result<(), AuthError> {
        redis::cmd("DEL")
            .arg(self.key(token))
            .query_async::<()>(&mut self.conn())
            .await
            .map_err(store_err)
    }

    pub async fn remove_subject(&self, subject: &str) -> Result<(), AuthError> {
        self.remove_subject_keys(subject, None).await
    }

    pub async fn remove_subject_except(
        &self,
        subject: &str,
        keep_token: &str,
    ) -> Result<(), AuthError> {
        self.remove_subject_keys(subject, Some(&self.key(keep_token)))
            .await
    }

    pub fn token<'a>(&self, headers: &'a HeaderMap) -> Option<&'a str> {
        read_cookie(headers, &self.inner.config.cookie_name).filter(|value| is_token(value))
    }

    pub fn cookie(&self, token: &str) -> String {
        cookie_value(&self.inner.config, token, Utc::now())
    }

    pub fn clear_cookie(&self) -> String {
        clear_value(&self.inner.config.cookie_name)
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

    async fn remove_subject_keys(
        &self,
        subject: &str,
        keep_key: Option<&str>,
    ) -> Result<(), AuthError> {
        let mut conn = self.conn();
        let pattern = format!("{}*", escape_glob(&self.inner.config.prefix));
        let mut keys = Vec::new();
        {
            let mut iter = conn
                .scan_match::<_, String>(pattern)
                .await
                .map_err(store_err)?;
            while let Some(key) = iter.next_item().await {
                keys.push(key.map_err(store_err)?);
            }
        }
        for key in keys {
            if keep_key == Some(key.as_str()) {
                continue;
            }
            let value: Option<String> = redis::cmd("GET")
                .arg(&key)
                .query_async(&mut conn)
                .await
                .map_err(store_err)?;
            let owner = value
                .as_deref()
                .and_then(parse_value)
                .map(|(_, owner)| owner);
            if owner == Some(subject) {
                redis::cmd("DEL")
                    .arg(&key)
                    .query_async::<()>(&mut conn)
                    .await
                    .map_err(store_err)?;
            }
        }
        Ok(())
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

fn random_token() -> Result<String, AuthError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|err| {
        AuthError::new(
            AuthErrorKind::Random,
            format!("os random source failed: {err}"),
        )
    })?;
    Ok(hex::encode(bytes))
}

fn escape_glob(prefix: &str) -> String {
    let mut out = String::with_capacity(prefix.len());
    for ch in prefix.chars() {
        if matches!(ch, '*' | '?' | '[' | ']' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

// A token from `create` is 64 lowercase hex characters. Other values skip the Redis call.
fn is_token(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn cookie_value(config: &SessionConfig, token: &str, now: DateTime<Utc>) -> String {
    let ttl = config.ttl_secs;
    let expires = now + chrono::Duration::seconds(i64::try_from(ttl).unwrap_or(i64::MAX / 1000));
    let stamp = expires.format("%a, %d %b %Y %H:%M:%S GMT");
    format!(
        "{}={token}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={ttl}; Expires={stamp}",
        config.cookie_name
    )
}

fn clear_value(cookie_name: &str) -> String {
    format!(
        "{cookie_name}=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT"
    )
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
    async fn remove_subject_except_keeps_one_session() {
        let Some(store) = store_or_skip().await else {
            return;
        };
        let current = store.create("7").await.unwrap();
        let old = store.create("7").await.unwrap();
        let other = store.create("8").await.unwrap();

        store.remove_subject_except("7", &current).await.unwrap();
        assert_eq!(store.subject(&current).await.unwrap().as_deref(), Some("7"));
        assert_eq!(store.subject(&old).await.unwrap(), None);
        assert_eq!(store.subject(&other).await.unwrap().as_deref(), Some("8"));

        store.remove_subject_except("8", &current).await.unwrap();
        assert_eq!(store.subject(&current).await.unwrap().as_deref(), Some("7"));
        assert_eq!(store.subject(&other).await.unwrap(), None);
        store.remove(&current).await.unwrap();
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

    #[test]
    fn tls_url_is_accepted() {
        assert!(redis::Client::open("rediss://default:secret@example.upstash.io:6379").is_ok());
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

    #[test]
    fn token_is_64_lowercase_hex_characters() {
        let token = random_token().unwrap();
        assert_eq!(token.len(), 64);
        assert!(token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
        assert_ne!(token, random_token().unwrap());
    }

    #[test]
    fn glob_characters_in_the_prefix_are_escaped() {
        assert_eq!(escape_glob("ics:session:"), "ics:session:");
        assert_eq!(escape_glob("a*b?[c]\\"), "a\\*b\\?\\[c\\]\\\\");
    }

    #[test]
    fn accepts_only_the_token_shape() {
        assert!(is_token(&"a1".repeat(32)));
        assert!(!is_token(&"a1".repeat(31)));
        assert!(!is_token(&"A1".repeat(32)));
        assert!(!is_token(&"g1".repeat(32)));
        assert!(!is_token(""));
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
            cookie_value(&config, "abc", now),
            "sid=abc; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=3600; Expires=Sat, 24 Oct 2026 11:00:00 GMT"
        );
    }

    #[test]
    fn clear_cookie_expires_at_once() {
        assert_eq!(
            clear_value("ics_session"),
            "ics_session=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT"
        );
    }
}
