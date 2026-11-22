use axum::http::HeaderMap;
use redis::aio::ConnectionManager;
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::Arc;

use crate::auth::session::config::SessionConfig;
use crate::auth::session::{Session, token};
use crate::exception;
use crate::exception::Exception;
use crate::http::headers::read_cookie;

pub const SESSION_STORE_COMMAND_FAILED: &str = "SESSION_STORE_COMMAND_FAILED";

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
        Ok(Self {
            inner: Arc::new(Inner { conn, config }),
        })
    }

    pub async fn create(&self, subject: &str) -> Result<Session, Exception> {
        let token = token::random_token()?;
        let key = self.key(&token);
        let subject_key = self.subject_key(subject);
        let (size,): (usize,) = redis::pipe()
            .atomic()
            .cmd("SET")
            .arg(&key)
            .arg(subject)
            .arg("EX")
            .arg(self.inner.config.ttl_secs)
            .ignore()
            .cmd("SADD")
            .arg(&subject_key)
            .arg(&key)
            .ignore()
            .cmd("SCARD")
            .arg(&subject_key)
            .query_async(&mut self.conn())
            .await
            .map_err(store_err)?;
        if size > self.inner.config.max_sessions_per_subject {
            self.trim_subject(&subject_key, &key).await?;
        }
        Ok(Session {
            token,
            subject: subject.to_string(),
        })
    }

    pub async fn load(&self, token: &str) -> Result<Option<Session>, Exception> {
        let subject: Option<String> = redis::cmd("GETEX")
            .arg(self.key(token))
            .arg("EX")
            .arg(self.inner.config.ttl_secs)
            .query_async(&mut self.conn())
            .await
            .map_err(store_err)?;
        Ok(subject.map(|subject| Session {
            token: token.to_string(),
            subject,
        }))
    }

    pub async fn remove(&self, token: &str) -> Result<(), Exception> {
        let key = self.key(token);
        let mut conn = self.conn();
        let subject: Option<String> = redis::cmd("GETDEL")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(store_err)?;
        let Some(subject) = subject else {
            return Ok(());
        };
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

    pub fn token<'a>(&self, headers: &'a HeaderMap) -> Option<&'a str> {
        read_cookie(headers, &self.inner.config.cookie_name).filter(|value| token::is_token(value))
    }

    pub fn cookie(&self, token: &str) -> String {
        self.inner.config.cookie(token)
    }

    pub fn clear_cookie(&self) -> String {
        self.inner.config.clear_cookie()
    }

    pub fn ttl_secs(&self) -> u64 {
        self.inner.config.ttl_secs
    }

    pub fn cookie_name(&self) -> &str {
        &self.inner.config.cookie_name
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

    async fn trim_subject(&self, subject_key: &str, new_key: &str) -> Result<(), Exception> {
        let mut conn = self.conn();
        let members: Vec<String> = redis::cmd("SMEMBERS")
            .arg(subject_key)
            .query_async(&mut conn)
            .await
            .map_err(store_err)?;
        let mut ttl_pipe = redis::pipe();
        for member in &members {
            ttl_pipe.cmd("TTL").arg(member);
        }
        let ttls: Vec<i64> = ttl_pipe.query_async(&mut conn).await.map_err(store_err)?;
        let mut gone = Vec::new();
        let mut live = Vec::new();
        for (member, ttl) in members.iter().zip(ttls) {
            if ttl < 0 {
                gone.push(member);
            } else if member != new_key {
                live.push((ttl, member));
            }
        }
        live.sort();
        let excess = (live.len() + 1).saturating_sub(self.inner.config.max_sessions_per_subject);
        let ended: Vec<&String> = live
            .iter()
            .take(excess)
            .map(|(_, member)| *member)
            .collect();
        let mut pipe = redis::pipe();
        pipe.atomic();
        if !ended.is_empty() {
            pipe.cmd("DEL").arg(&ended).ignore();
        }
        gone.extend(ended);
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
}

fn store_err(err: redis::RedisError) -> Exception {
    exception!(
        code = SESSION_STORE_COMMAND_FAILED,
        "session store command failed",
        source = err,
    )
}
