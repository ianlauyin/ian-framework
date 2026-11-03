mod error;
mod password;
mod session_config;
mod session_store;

pub use error::{AuthError, AuthErrorKind};
pub use password::{hash_password, verify_password};
pub use session_config::SessionConfig;
pub use session_store::SessionStore;

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) async fn store_or_skip() -> Option<SessionStore> {
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
}
