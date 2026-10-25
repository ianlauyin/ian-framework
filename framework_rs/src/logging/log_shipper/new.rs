use std::collections::VecDeque;
use std::env;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{Buffer, Inner, LogShipper, DEFAULT_CAPACITY, SHIP_TIMEOUT};

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

impl LogShipper {
    pub fn new(config: ShipperConfig) -> Option<Self> {
        let mut chars = config.app.chars();
        let first = chars.next()?;
        if !first.is_ascii_alphanumeric()
            || !config
                .app
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
            || config.capacity == 0
            || config.ship_every.is_zero()
        {
            return None;
        }
        let endpoint = match config.endpoint {
            None => None,
            Some(raw) => {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    None
                } else if trimmed.chars().any(char::is_whitespace) {
                    return None;
                } else {
                    let parsed = reqwest::Url::parse(trimmed).ok()?;
                    if (parsed.scheme() != "http" && parsed.scheme() != "https")
                        || parsed.host_str().is_none()
                        || parsed.query().is_some()
                        || parsed.fragment().is_some()
                    {
                        return None;
                    }
                    Some(parsed.to_string().trim_end_matches('/').to_string())
                }
            }
        };
        let masks = config
            .mask_patterns
            .iter()
            .map(|pattern| regex::Regex::new(pattern).ok())
            .collect::<Option<Vec<_>>>()?;
        let http = match &endpoint {
            Some(_) => Some(
                reqwest::Client::builder()
                    .connect_timeout(SHIP_TIMEOUT)
                    .timeout(SHIP_TIMEOUT)
                    .redirect(reqwest::redirect::Policy::none())
                    .user_agent(concat!("ian_framework/", env!("CARGO_PKG_VERSION")))
                    .build()
                    .ok()?,
            ),
            None => None,
        };
        if endpoint.is_some() {
            tracing::info!(app = %config.app, "log shipping posts to the log server");
        } else {
            tracing::info!(app = %config.app, "log shipping is trace-only");
        }
        Some(Self {
            inner: Arc::new(Inner {
                app: config.app,
                endpoint,
                capacity: config.capacity,
                ship_every: config.ship_every,
                masks,
                buffer: Mutex::new(Buffer {
                    events: VecDeque::new(),
                    capacity_warned: false,
                }),
                http,
                shipping: tokio::sync::Mutex::new(()),
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::logging::log_shipper::{DEFAULT_CAPACITY, SHIP_TIMEOUT};
    use crate::logging::{LogShipper, ShipperConfig, SHIP_CHUNK};

    use super::ENDPOINT_ENV;

    fn config(app: &str, endpoint: &str) -> ShipperConfig {
        let mut config = ShipperConfig::new(app);
        config.endpoint = Some(endpoint.to_string());
        config
    }

    #[test]
    fn defaults() {
        let config = ShipperConfig::new("api");
        assert_eq!(config.endpoint, std::env::var(ENDPOINT_ENV).ok());
        assert_eq!(ENDPOINT_ENV, "LOG_ENDPOINT");
        assert_eq!(
            config.mask_patterns,
            [r"(?i)password", r"(?i)secret", r"(?i)token"]
        );
        assert_eq!(config.ship_every, Duration::from_secs(5));
        let shipper = LogShipper::new(config).unwrap();
        assert_eq!(shipper.inner.app, "api");
        assert_eq!(shipper.inner.masks.len(), 3);
        assert_eq!(DEFAULT_CAPACITY, 10_000);
        assert_eq!(SHIP_CHUNK, 1_000);
        assert_eq!(SHIP_TIMEOUT, Duration::from_secs(10));
    }

    #[test]
    fn blank_endpoint_is_trace_only() {
        for endpoint in ["", "   "] {
            let shipper = LogShipper::new(config("api", endpoint)).unwrap();
            assert_eq!(shipper.inner.endpoint, None);
        }
        let mut unset = ShipperConfig::new("api");
        unset.endpoint = None;
        assert_eq!(LogShipper::new(unset).unwrap().inner.endpoint, None);
    }

    #[test]
    fn endpoint_is_normalized() {
        let shipper = LogShipper::new(config("cs", "http://logs.example/ingest/")).unwrap();
        assert_eq!(
            shipper.inner.endpoint.as_deref(),
            Some("http://logs.example/ingest")
        );
    }

    #[test]
    fn invalid_config_is_rejected() {
        let cases = [
            config("", "http://logs.example"),
            config("a/b", "http://logs.example"),
            {
                let mut config = config("api", "http://logs.example");
                config.capacity = 0;
                config
            },
            {
                let mut config = config("api", "http://logs.example");
                config.ship_every = Duration::ZERO;
                config
            },
            config("api", "ftp://logs.example"),
            config("api", "http://logs.example/a b"),
            {
                let mut config = config("api", "http://logs.example");
                config.mask_patterns = vec!["(".to_string()];
                config
            },
        ];
        for config in cases {
            assert!(LogShipper::new(config).is_none());
        }
    }
}
