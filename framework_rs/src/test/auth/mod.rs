mod password;
mod session;

use crate::auth::*;

pub(crate) const LIMIT: usize = 5;

pub(crate) async fn store_or_skip() -> Option<SessionStore> {
    let conn = connection().await?;
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).unwrap();
    let config = SessionConfig {
        prefix: format!("ian-framework-test:{}:", hex::encode(bytes)),
        cookie_name: "sid".to_string(),
        ttl_secs: 60,
        max_sessions_per_subject: LIMIT,
    };
    SessionStore::new(conn, config).ok()
}

pub(crate) async fn connection() -> Option<redis::aio::ConnectionManager> {
    let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    match crate::redis::connect(&url).await {
        Ok(conn) => Some(conn),
        Err(err) => {
            eprintln!("skipping session store test: {err}");
            None
        }
    }
}
