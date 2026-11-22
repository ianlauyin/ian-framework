use axum::http::HeaderMap;

use crate::auth::session::token;
use crate::http::headers::read_cookie;

const ATTRIBUTES: &str = "HttpOnly; Secure; SameSite=Lax; Path=/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionCookie {
    name: String,
    ttl_secs: u64,
}

impl SessionCookie {
    pub fn new(name: impl Into<String>, ttl_secs: u64) -> Self {
        Self {
            name: name.into(),
            ttl_secs,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn read<'a>(&self, headers: &'a HeaderMap) -> Option<&'a str> {
        read_cookie(headers, &self.name).filter(|value| token::is_token(value))
    }

    pub fn set(&self, token: &str) -> String {
        format!(
            "{}={token}; {ATTRIBUTES}; Max-Age={}",
            self.name, self.ttl_secs
        )
    }

    pub fn clear(&self) -> String {
        format!("{}=; {ATTRIBUTES}; Max-Age=0", self.name)
    }
}
