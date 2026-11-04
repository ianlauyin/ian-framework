use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::http::HeaderMap;
use chrono::Utc;
use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use sha2::{Digest, Sha256};

use crate::auth::SessionConfig;
use crate::exception;
use crate::exception::Exception;
use crate::http::headers::read_cookie;

// A subject that logs in again and again must not make the Redis set grow without a bound.
pub(crate) const MAX_SESSIONS_PER_SUBJECT: usize = 100;

// Sets the expiry to the smaller of the ttl and the time to the deadline in one step.
const TOUCH_SCRIPT: &str = r#"
local value = redis.call('GET', KEYS[1])
if not value then return 0 end
local deadline = tonumber(string.match(value, '^(%d+):'))
if not deadline then return 0 end
local left = deadline - tonumber(ARGV[1])
if left <= 0 then return 0 end
return redis.call('EXPIRE', KEYS[1], math.min(tonumber(ARGV[2]), left))
"#;

#[derive(Clone)]
pub struct SessionStore {
    pub(crate) inner: Arc<Inner>,
}

impl SessionStore {
    pub fn ttl_secs(&self) -> u64 {
        self.inner.config.ttl_secs
    }

    pub async fn connect(url: &str, config: SessionConfig) -> Result<Self, Exception> {
        config.check()?;
        let client = redis::Client::open(url).map_err(|err| {
            exception!("redis url is not valid", code = "AUTH_CONFIG", source = err)
        })?;
        let mut conn = ConnectionManager::new_with_config(client, manager_config())
            .await
            .map_err(unreachable)?;
        let pong: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(unreachable)?;
        if pong != "PONG" {
            return Err(exception!("cannot reach redis", code = "AUTH_STORE"));
        }
        Ok(Self {
            inner: Arc::new(Inner { conn, config }),
        })
    }

    pub async fn create(&self, subject: &str) -> Result<String, Exception> {
        let token = random_token()?;
        let max_age = self.inner.config.max_age_secs;
        let deadline = now_secs().saturating_add(max_age);
        let key = self.key(&token);
        let subject_key = self.subject_key(subject);
        redis::pipe()
            .atomic()
            .cmd("SET")
            .arg(&key)
            .arg(format!("{deadline}:{subject}"))
            .arg("EX")
            .arg(self.ttl_secs().min(max_age))
            .ignore()
            .cmd("SADD")
            .arg(&subject_key)
            .arg(&key)
            .ignore()
            .cmd("EXPIRE")
            .arg(&subject_key)
            .arg(max_age)
            .ignore()
            .query_async::<()>(&mut self.conn())
            .await
            .map_err(store_err)?;
        self.trim_subject(&subject_key, &key).await?;
        Ok(token)
    }

    pub async fn subject(&self, token: &str) -> Result<Option<String>, Exception> {
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

    pub async fn touch(&self, token: &str) -> Result<bool, Exception> {
        redis::cmd("EVAL")
            .arg(TOUCH_SCRIPT)
            .arg(1)
            .arg(self.key(token))
            .arg(now_secs())
            .arg(self.ttl_secs())
            .query_async(&mut self.conn())
            .await
            .map_err(store_err)
    }

    pub async fn remove(&self, token: &str) -> Result<(), Exception> {
        let key = self.key(token);
        let mut conn = self.conn();
        let value: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(store_err)?;
        let mut pipe = redis::pipe();
        pipe.atomic().cmd("DEL").arg(&key).ignore();
        if let Some((_, subject)) = value.as_deref().and_then(parse_value) {
            pipe.cmd("SREM")
                .arg(self.subject_key(subject))
                .arg(&key)
                .ignore();
        }
        pipe.query_async::<()>(&mut conn).await.map_err(store_err)
    }

    pub async fn remove_subject(&self, subject: &str) -> Result<(), Exception> {
        self.remove_subject_keys(subject, None).await
    }

    pub async fn remove_subject_except(
        &self,
        subject: &str,
        keep_token: &str,
    ) -> Result<(), Exception> {
        self.remove_subject_keys(subject, Some(&self.key(keep_token)))
            .await
    }

    pub fn token<'a>(&self, headers: &'a HeaderMap) -> Option<&'a str> {
        read_cookie(headers, &self.inner.config.cookie_name).filter(|value| is_token(value))
    }

    pub fn cookie(&self, token: &str) -> String {
        self.inner.config.cookie(token, Utc::now())
    }

    pub fn clear_cookie(&self) -> String {
        self.inner.config.clear_cookie()
    }

    pub(crate) fn cookie_name(&self) -> &str {
        &self.inner.config.cookie_name
    }

    pub(crate) fn conn(&self) -> ConnectionManager {
        self.inner.conn.clone()
    }

    // A copy of Redis must not give working tokens, so the key holds only the token hash.
    pub(crate) fn key(&self, token: &str) -> String {
        let hash = hex::encode(Sha256::digest(token.as_bytes()));
        format!("{}{hash}", self.inner.config.prefix)
    }

    // The set holds the keys of the sessions of one subject. It is hashed so the subject cannot change the key.
    pub(crate) fn subject_key(&self, subject: &str) -> String {
        let hash = hex::encode(Sha256::digest(subject.as_bytes()));
        format!("{}subject:{hash}", self.inner.config.prefix)
    }

    // The set also holds the keys of expired sessions. Those go first, and then the sessions that remain over the limit, but never `keep_key`.
    async fn trim_subject(&self, subject_key: &str, keep_key: &str) -> Result<(), Exception> {
        let mut conn = self.conn();
        let size: usize = redis::cmd("SCARD")
            .arg(subject_key)
            .query_async(&mut conn)
            .await
            .map_err(store_err)?;
        if size <= MAX_SESSIONS_PER_SUBJECT {
            return Ok(());
        }
        let members: Vec<String> = redis::cmd("SMEMBERS")
            .arg(subject_key)
            .query_async(&mut conn)
            .await
            .map_err(store_err)?;
        let mut live = Vec::new();
        let mut gone = Vec::new();
        for member in members {
            let exists: bool = redis::cmd("EXISTS")
                .arg(&member)
                .query_async(&mut conn)
                .await
                .map_err(store_err)?;
            if exists {
                live.push(member);
            } else {
                gone.push(member);
            }
        }
        let excess = live.len().saturating_sub(MAX_SESSIONS_PER_SUBJECT);
        let dropped: Vec<String> = live
            .into_iter()
            .filter(|member| member != keep_key)
            .take(excess)
            .collect();
        let mut pipe = redis::pipe();
        pipe.atomic();
        for member in &dropped {
            pipe.cmd("DEL").arg(member).ignore();
        }
        gone.extend(dropped);
        if gone.is_empty() {
            return Ok(());
        }
        pipe.cmd("SREM").arg(subject_key).arg(&gone).ignore();
        pipe.query_async::<()>(&mut conn).await.map_err(store_err)
    }

    async fn remove_subject_keys(
        &self,
        subject: &str,
        keep_key: Option<&str>,
    ) -> Result<(), Exception> {
        let subject_key = self.subject_key(subject);
        let mut conn = self.conn();
        let members: Vec<String> = redis::cmd("SMEMBERS")
            .arg(&subject_key)
            .query_async(&mut conn)
            .await
            .map_err(store_err)?;
        let removed: Vec<&String> = members
            .iter()
            .filter(|key| keep_key != Some(key.as_str()))
            .collect();
        if removed.is_empty() {
            return Ok(());
        }
        let mut pipe = redis::pipe();
        pipe.atomic();
        for key in &removed {
            pipe.cmd("DEL").arg(key).ignore();
        }
        pipe.cmd("SREM").arg(&subject_key).arg(&removed).ignore();
        pipe.query_async::<()>(&mut conn).await.map_err(store_err)
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

pub(crate) struct Inner {
    conn: ConnectionManager,
    pub(crate) config: SessionConfig,
}

fn store_err(err: redis::RedisError) -> Exception {
    exception!(
        "session store command failed",
        code = "AUTH_STORE",
        source = err
    )
}

pub(crate) fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}

// The stored value is `{deadline}:{subject}`. The deadline is the end of the session in Unix seconds.
pub(crate) fn parse_value(value: &str) -> Option<(u64, &str)> {
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

fn unreachable(err: redis::RedisError) -> Exception {
    exception!("cannot reach redis", code = "AUTH_STORE", source = err)
}

pub(crate) fn random_token() -> Result<String, Exception> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|err| {
        exception!(
            format!("os random source failed: {err}"),
            code = "AUTH_RANDOM"
        )
    })?;
    Ok(hex::encode(bytes))
}

// A token from `create` is 64 lowercase hex characters. Other values skip the Redis call.
pub(crate) fn is_token(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
