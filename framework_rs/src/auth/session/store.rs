use redis::aio::ConnectionManager;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::Arc;

use crate::auth::session::config::SessionConfig;
use crate::auth::session::cookie::SessionCookie;
use crate::auth::session::{Session, token};
use crate::exception;
use crate::exception::Exception;
use crate::time::hkt_now;

pub const SESSION_STORE_COMMAND_FAILED: &str = "SESSION_STORE_COMMAND_FAILED";

const CREATE_SCRIPT: &str = r"
redis.call('SET', KEYS[1], ARGV[1], 'EX', ARGV[2])
redis.call('SADD', KEYS[2], KEYS[1])
redis.call('EXPIRE', KEYS[2], ARGV[2])
local max = tonumber(ARGV[3])
if redis.call('SCARD', KEYS[2]) <= max then
    return
end
local live = {}
for _, member in ipairs(redis.call('SMEMBERS', KEYS[2])) do
    local ttl = redis.call('TTL', member)
    if ttl < 0 then
        redis.call('SREM', KEYS[2], member)
    elseif member ~= KEYS[1] then
        live[#live + 1] = {ttl, member}
    end
end
table.sort(live, function(a, b)
    if a[1] ~= b[1] then
        return a[1] < b[1]
    end
    return a[2] < b[2]
end)
for index = 1, #live + 1 - max do
    redis.call('DEL', live[index][2])
    redis.call('SREM', KEYS[2], live[index][2])
end
";

const REMOVE_SUBJECT_SCRIPT: &str = r"
local members = redis.call('SMEMBERS', KEYS[1])
for _, member in ipairs(members) do
    if member ~= ARGV[1] then
        redis.call('DEL', member)
        redis.call('SREM', KEYS[1], member)
    end
end
";

#[derive(Clone)]
pub struct SessionStore {
    inner: Arc<Inner>,
}

impl SessionStore {
    pub fn new(conn: ConnectionManager, config: SessionConfig) -> Result<Self, Exception> {
        config.check()?;
        let cookie = SessionCookie::new(config.cookie_name.clone(), config.ttl_secs);
        Ok(Self {
            inner: Arc::new(Inner {
                conn,
                config,
                cookie,
            }),
        })
    }

    pub async fn create(&self, subject: &str) -> Result<Session, Exception> {
        let token = token::random_token()?;
        let stored = Stored {
            subject: subject.to_string(),
            expires_at: now_secs().saturating_add(self.inner.config.absolute_ttl_secs),
        };
        redis::Script::new(CREATE_SCRIPT)
            .key(self.key(&token))
            .key(self.subject_key(subject))
            .arg(stored.encode())
            .arg(self.inner.config.ttl_secs)
            .arg(self.inner.config.max_sessions_per_subject)
            .invoke_async::<()>(&mut self.conn())
            .await
            .map_err(store_err)?;
        Ok(Session {
            token,
            subject: stored.subject,
        })
    }

    pub async fn load(&self, token: &str) -> Result<Option<Session>, Exception> {
        let key = self.key(token);
        let mut conn = self.conn();
        let raw: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(store_err)?;
        let Some(raw) = raw else {
            return Ok(None);
        };
        let now = now_secs();
        let stored = match Stored::parse(&raw) {
            Some(stored) => stored,
            None => {
                let stored = Stored {
                    subject: raw,
                    expires_at: now.saturating_add(self.inner.config.absolute_ttl_secs),
                };
                redis::cmd("SET")
                    .arg(&key)
                    .arg(stored.encode())
                    .arg("XX")
                    .arg("KEEPTTL")
                    .query_async::<()>(&mut conn)
                    .await
                    .map_err(store_err)?;
                stored
            }
        };
        let left = stored.expires_at.saturating_sub(now);
        if left == 0 {
            self.remove(token).await?;
            return Ok(None);
        }
        let (live,): (bool,) = redis::pipe()
            .atomic()
            .cmd("EXPIRE")
            .arg(self.subject_key(&stored.subject))
            .arg(self.inner.config.ttl_secs)
            .ignore()
            .cmd("EXPIRE")
            .arg(&key)
            .arg(left.min(self.inner.config.ttl_secs))
            .query_async(&mut conn)
            .await
            .map_err(store_err)?;
        if !live {
            return Ok(None);
        }
        Ok(Some(Session {
            token: token.to_string(),
            subject: stored.subject,
        }))
    }

    pub async fn remove(&self, token: &str) -> Result<(), Exception> {
        let key = self.key(token);
        let mut conn = self.conn();
        let raw: Option<String> = redis::cmd("GETDEL")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(store_err)?;
        let Some(raw) = raw else {
            return Ok(());
        };
        let subject = Stored::parse(&raw).map_or(raw, |stored| stored.subject);
        redis::cmd("SREM")
            .arg(self.subject_key(&subject))
            .arg(&key)
            .query_async::<()>(&mut conn)
            .await
            .map_err(store_err)
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

    pub fn cookie(&self) -> &SessionCookie {
        &self.inner.cookie
    }

    pub fn conn(&self) -> ConnectionManager {
        self.inner.conn.clone()
    }

    pub fn key(&self, token: &str) -> String {
        let hash = hex::encode(Sha256::digest(token.as_bytes()));
        format!("{}{hash}", self.inner.config.prefix)
    }

    pub fn subject_key(&self, subject: &str) -> String {
        let hash = hex::encode(Sha256::digest(subject.as_bytes()));
        format!("{}subject:{hash}", self.inner.config.prefix)
    }

    async fn remove_subject_keys(
        &self,
        subject: &str,
        keep_key: Option<&str>,
    ) -> Result<(), Exception> {
        redis::Script::new(REMOVE_SUBJECT_SCRIPT)
            .key(self.subject_key(subject))
            .arg(keep_key.unwrap_or_default())
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
    cookie: SessionCookie,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    subject: String,
    expires_at: u64,
}

impl Stored {
    fn parse(raw: &str) -> Option<Self> {
        serde_json::from_str(raw).ok()
    }

    fn encode(&self) -> String {
        serde_json::json!({ "subject": self.subject, "expires_at": self.expires_at }).to_string()
    }
}

fn now_secs() -> u64 {
    u64::try_from(hkt_now().timestamp()).unwrap_or(0)
}

fn store_err(err: redis::RedisError) -> Exception {
    exception!(
        code = SESSION_STORE_COMMAND_FAILED,
        "session store command failed",
        source = err,
    )
}
