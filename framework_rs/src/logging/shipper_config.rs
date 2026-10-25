use std::env;
use std::time::Duration;

const DEFAULT_CAPACITY: usize = 10_000;
const ENDPOINT_ENV: &str = "LOG_ENDPOINT";
const DEFAULT_SHIP_EVERY: Duration = Duration::from_secs(5);
// log_request records the query and the JSON body, so a credential field must not reach a log.
const DEFAULT_MASK_PATTERNS: [&str; 3] = [r"(?i)password", r"(?i)secret", r"(?i)token"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipperConfig {
    pub app: String,
    pub endpoint: Option<String>,
    pub capacity: usize,
    pub mask_patterns: Vec<String>,
    pub ship_every: Duration,
}

impl ShipperConfig {
    pub fn new(app: impl Into<String>) -> Self {
        Self {
            app: app.into(),
            endpoint: env::var(ENDPOINT_ENV).ok(),
            capacity: DEFAULT_CAPACITY,
            mask_patterns: DEFAULT_MASK_PATTERNS
                .iter()
                .map(|pattern| pattern.to_string())
                .collect(),
            ship_every: DEFAULT_SHIP_EVERY,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
