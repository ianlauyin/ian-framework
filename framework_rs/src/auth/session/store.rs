use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use axum::http::HeaderMap;
use chrono::Utc;
use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use redis::Script;
use sha2::{Digest, Sha256};

use crate::auth::session::config::SessionConfig;
use crate::auth::session::{token, Session};
use crate::exception;
use crate::exception::Exception;
use crate::http::headers::read_cookie;
use crate::time::now_secs;

// A subject that logs in again and again must not make the Redis set grow without a bound.
pub(crate) const MAX_SESSIONS_PER_SUBJECT: usize = 100;

// KEYS: session key, subject set key. ARGV: value, session expiry, set expiry, session limit.
// Over the limit, the keys of expired sessions leave the set first. Then the oldest live sessions end, but never the new one.
const CREATE_SCRIPT: &str = r#"
redis.call('SET', KEYS[1], ARGV[1], 'EX', ARGV[2])
redis.call('SADD', KEYS[2], KEYS[1])
redis.call('EXPIRE', KEYS[2], ARGV[3])
local limit = tonumber(ARGV[4])
if redis.call('SCARD', KEYS[2]) <= limit then return 0 end
local live = {}
for _, member in ipairs(redis.call('SMEMBERS', KEYS[2])) do
  local value = redis.call('GET', member)
  if not value then
    redis.call('SREM', KEYS[2], member)
  elseif member ~= KEYS[1] then
    table.insert(live, {member, tonumber(string.match(value, '^(%d+):')) or 0})
  end
end
table.sort(live, function(a, b) return a[2] < b[2] end)
for i = 1, #live + 1 - limit do
  redis.call('DEL', live[i][1])
  redis.call('SREM', KEYS[2], live[i][1])
end
return 0
"#;

// KEYS: session key. ARGV: now, ttl.
// Returns the subject and the new expiry, which is the smaller of the ttl and the time to the deadline.
const LOAD_SCRIPT: &str = r#"
local value = redis.call('GET', KEYS[1])
if not value then return false end
local deadline, subject = string.match(value, '^(%d+):(.*)$')
if not deadline then return false end
local left = tonumber(deadline) - tonumber(ARGV[1])
if left <= 0 then return false end
local expiry = math.min(tonumber(ARGV[2]), left)
redis.call('EXPIRE', KEYS[1], expiry)
return {subject, expiry}
"#;

// KEYS: subject set key. ARGV: the session key to keep, or an empty string.
const REMOVE_SUBJECT_SCRIPT: &str = r#"
for _, member in ipairs(redis.call('SMEMBERS', KEYS[1])) do
  if member ~= ARGV[1] then
    redis.call('DEL', member)
    redis.call('SREM', KEYS[1], member)
  end
end
return 0
"#;

#[derive(Clone)]
pub struct SessionStore {
    inner: Arc<Inner>,
}

impl SessionStore {
    pub async fn connect(url: &str, config: SessionConfig) -> Result<Self, Exception> {
        config.check()?;
        let client = redis::Client::open(url).map_err(|err| {
            exception!(code = "AUTH_CONFIG", "redis url is not valid", source = err)
        })?;
        let mut conn = ConnectionManager::new_with_config(client, manager_config())
            .await
            .map_err(unreachable)?;
        let pong: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(unreachable)?;
        if pong != "PONG" {
            return Err(exception!(code = "AUTH_STORE", "cannot reach redis"));
        }
        Ok(Self {
            inner: Arc::new(Inner {
                conn,
                config,
                create_script: Script::new(CREATE_SCRIPT),
                load_script: Script::new(LOAD_SCRIPT),
                remove_subject_script: Script::new(REMOVE_SUBJECT_SCRIPT),
            }),
        })
    }

    pub async fn create(&self, subject: &str) -> Result<Session, Exception> {
        let token = token::random_token()?;
        let config = &self.inner.config;
        let deadline = now_secs().saturating_add(config.max_age_secs);
        self.inner
            .create_script
            .key(self.key(&token))
            .key(self.subject_key(subject))
            .arg(format!("{deadline}:{subject}"))
            .arg(config.ttl_secs)
            .arg(config.max_age_secs)
            .arg(MAX_SESSIONS_PER_SUBJECT)
            .invoke_async::<()>(&mut self.conn())
            .await
            .map_err(store_err)?;
        Ok(Session {
            token,
            subject: subject.to_string(),
            expires_in_secs: config.ttl_secs,
        })
    }

    pub async fn load(&self, token: &str) -> Result<Option<Session>, Exception> {
        let found: Option<(String, u64)> = self
            .inner
            .load_script
            .key(self.key(token))
            .arg(now_secs())
            .arg(self.inner.config.ttl_secs)
            .invoke_async(&mut self.conn())
            .await
            .map_err(store_err)?;
        Ok(found.map(|(subject, expires_in_secs)| Session {
            token: token.to_string(),
            subject,
            expires_in_secs,
        }))
    }

    // The key of an ended session can stay in the subject set. `create` and `remove_subject` clear it.
    pub async fn remove(&self, token: &str) -> Result<(), Exception> {
        redis::cmd("DEL")
            .arg(self.key(token))
            .query_async::<()>(&mut self.conn())
            .await
            .map_err(store_err)
    }

    pub async fn remove_subject(&self, subject: &str) -> Result<(), Exception> {
        self.remove_subject_keys(subject, "").await
    }

    pub async fn remove_subject_except(
        &self,
        subject: &str,
        keep_token: &str,
    ) -> Result<(), Exception> {
        self.remove_subject_keys(subject, &self.key(keep_token))
            .await
    }

    pub fn token<'a>(&self, headers: &'a HeaderMap) -> Option<&'a str> {
        read_cookie(headers, &self.inner.config.cookie_name).filter(|value| token::is_token(value))
    }

    pub fn cookie(&self, session: &Session) -> String {
        self.inner
            .config
            .cookie(&session.token, session.expires_in_secs, Utc::now())
    }

    pub fn clear_cookie(&self) -> String {
        self.inner.config.clear_cookie()
    }

    pub fn ttl_secs(&self) -> u64 {
        self.inner.config.ttl_secs
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

    async fn remove_subject_keys(&self, subject: &str, keep_key: &str) -> Result<(), Exception> {
        self.inner
            .remove_subject_script
            .key(self.subject_key(subject))
            .arg(keep_key)
            .invoke_async::<()>(&mut self.conn())
            .await
            .map_err(store_err)
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
    create_script: Script,
    load_script: Script,
    remove_subject_script: Script,
}

fn store_err(err: redis::RedisError) -> Exception {
    exception!(
        code = "AUTH_STORE",
        "session store command failed",
        source = err,
    )
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
    exception!(code = "AUTH_STORE", "cannot reach redis", source = err)
}
