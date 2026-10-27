use std::fmt;
use std::time::Duration;

const DEFAULT_CAPACITY: usize = 10_000;
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
            endpoint: None,
            post_api_key: None,
            capacity: DEFAULT_CAPACITY,
            mask_patterns: DEFAULT_MASK_PATTERNS
                .iter()
                .map(|pattern| pattern.to_string())
                .collect(),
            ship_every: DEFAULT_SHIP_EVERY,
        }
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
    use super::*;

    #[test]
    fn defaults() {
        let config = ShipperConfig::new("my-app");
        assert_eq!(config.app, "my-app");
        assert_eq!(config.endpoint, None);
        assert_eq!(config.post_api_key, None);
        assert_eq!(config.capacity, DEFAULT_CAPACITY);
        assert_eq!(DEFAULT_CAPACITY, 10_000);
        assert_eq!(
            config.mask_patterns,
            [r"(?i)password", r"(?i)secret", r"(?i)token"]
        );
        assert_eq!(config.ship_every, Duration::from_secs(5));
    }

    #[test]
    fn debug_hides_the_post_api_key() {
        let mut config = ShipperConfig::new("my-app");
        config.post_api_key = Some("my-post-key".to_string());
        let debug = format!("{config:?}");
        assert!(!debug.contains("my-post-key"));
        assert!(debug.contains("post_api_key: Some(\"***\")"));
        assert!(debug.contains("app: \"my-app\""));
    }
}
