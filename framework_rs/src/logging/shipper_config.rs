use std::fmt;
use std::time::Duration;

use crate::env;
use crate::env::EnvError;

const DEFAULT_CAPACITY: usize = 10_000;
const ENDPOINT_ENV: &str = "LOG_ENDPOINT";
const POST_API_KEY_ENV: &str = "LOG_POST_API_KEY";
const DEFAULT_SHIP_EVERY: Duration = Duration::from_secs(5);
// log_request records the query and the JSON body, so a credential field must not reach a log.
const DEFAULT_MASK_PATTERNS: [&str; 3] = [r"(?i)password", r"(?i)secret", r"(?i)token"];

#[derive(Clone, PartialEq, Eq)]
pub struct ShipperConfig {
    pub app: String,
    pub endpoint: Option<String>,
    pub post_api_key: Option<String>,
    pub capacity: usize,
    pub mask_patterns: Vec<String>,
    pub ship_every: Duration,
}

impl ShipperConfig {
    pub fn new(app: impl Into<String>) -> Self {
        Self {
            app: app.into(),
            endpoint: std::env::var(ENDPOINT_ENV).ok(),
            post_api_key: std::env::var(POST_API_KEY_ENV).ok(),
            capacity: DEFAULT_CAPACITY,
            mask_patterns: DEFAULT_MASK_PATTERNS
                .iter()
                .map(|pattern| pattern.to_string())
                .collect(),
            ship_every: DEFAULT_SHIP_EVERY,
        }
    }

    pub async fn from_env(app: impl Into<String>) -> Result<Self, EnvError> {
        let mut config = Self::new(app);
        config.post_api_key = env::optional(POST_API_KEY_ENV).await?;
        Ok(config)
    }
}

impl fmt::Debug for ShipperConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShipperConfig")
            .field("app", &self.app)
            .field("endpoint", &self.endpoint)
            .field("post_api_key", &self.post_api_key.as_ref().map(|_| "***"))
            .field("capacity", &self.capacity)
            .field("mask_patterns", &self.mask_patterns)
            .field("ship_every", &self.ship_every)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;
    use crate::env::tests::{block_on, with_var};

    #[test]
    fn defaults() {
        let config = ShipperConfig::new("api");
        assert_eq!(config.endpoint, std::env::var(ENDPOINT_ENV).ok());
        assert_eq!(ENDPOINT_ENV, "LOG_ENDPOINT");
        assert_eq!(config.capacity, DEFAULT_CAPACITY);
        assert_eq!(DEFAULT_CAPACITY, 10_000);
        assert_eq!(
            config.mask_patterns,
            [r"(?i)password", r"(?i)secret", r"(?i)token"]
        );
        assert_eq!(config.ship_every, Duration::from_secs(5));
    }

    fn with_post_api_key<T>(value: Option<&str>, read: impl FnOnce() -> T) -> T {
        with_var(POST_API_KEY_ENV, value.map(OsStr::new), read)
    }

    #[test]
    fn post_api_key_is_read_from_the_env() {
        assert_eq!(POST_API_KEY_ENV, "LOG_POST_API_KEY");
        let set = with_post_api_key(Some("log-post-key"), || ShipperConfig::new("api"));
        assert_eq!(set.post_api_key.as_deref(), Some("log-post-key"));
        let unset = with_post_api_key(None, || ShipperConfig::new("api"));
        assert_eq!(unset.post_api_key, None);
    }

    fn from_env_with(key: Option<&str>) -> Result<ShipperConfig, EnvError> {
        with_post_api_key(key, || block_on(ShipperConfig::from_env("api")))
    }

    #[test]
    fn from_env_reads_the_trimmed_key_or_none() {
        let set = from_env_with(Some(" log-post-key ")).unwrap();
        assert_eq!(set.app, "api");
        assert_eq!(set.post_api_key.as_deref(), Some("log-post-key"));
        let unset = from_env_with(None).unwrap();
        assert_eq!(unset.post_api_key, None);
        let blank = from_env_with(Some(" ")).unwrap();
        assert_eq!(blank.post_api_key, None);
    }

    #[test]
    fn debug_hides_the_post_api_key() {
        let mut config = ShipperConfig::new("api");
        config.post_api_key = Some("log-post-key".to_string());
        let debug = format!("{config:?}");
        assert!(!debug.contains("log-post-key"));
        assert!(debug.contains("post_api_key: Some(\"***\")"));
        assert!(debug.contains("app: \"api\""));
    }
}
