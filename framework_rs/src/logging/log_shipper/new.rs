use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::{Buffer, Inner, LogShipper, DEFAULT_CAPACITY, SHIP_TIMEOUT};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipperConfig {
    pub app: String,
    pub endpoint: Option<String>,
    pub capacity: usize,
}

impl ShipperConfig {
    pub fn new(app: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self {
            app: app.into(),
            endpoint: Some(endpoint.into()),
            capacity: DEFAULT_CAPACITY,
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
        Some(Self {
            inner: Arc::new(Inner {
                app: config.app,
                endpoint,
                capacity: config.capacity,
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

    #[test]
    fn defaults_are_buffer_only() {
        let shipper = LogShipper::new(ShipperConfig::new("api", "")).unwrap();
        assert_eq!(shipper.inner.app, "api");
        assert_eq!(shipper.inner.endpoint, None);
        assert_eq!(DEFAULT_CAPACITY, 10_000);
        assert_eq!(SHIP_CHUNK, 1_000);
        assert_eq!(SHIP_TIMEOUT, Duration::from_secs(10));
    }

    #[test]
    fn blank_endpoint_is_buffer_only() {
        for endpoint in ["", "   "] {
            let shipper = LogShipper::new(ShipperConfig::new("api", endpoint)).unwrap();
            assert_eq!(shipper.inner.endpoint, None);
        }
    }

    #[test]
    fn endpoint_is_normalized() {
        let shipper =
            LogShipper::new(ShipperConfig::new("cs", "http://logs.example/ingest/")).unwrap();
        assert_eq!(
            shipper.inner.endpoint.as_deref(),
            Some("http://logs.example/ingest")
        );
    }

    #[test]
    fn invalid_config_is_rejected() {
        let cases = [
            ShipperConfig::new("", "http://logs.example"),
            ShipperConfig::new("a/b", "http://logs.example"),
            {
                let mut config = ShipperConfig::new("api", "http://logs.example");
                config.capacity = 0;
                config
            },
            ShipperConfig::new("api", "ftp://logs.example"),
            ShipperConfig::new("api", "http://logs.example/a b"),
        ];
        for config in cases {
            assert!(LogShipper::new(config).is_none());
        }
    }
}
