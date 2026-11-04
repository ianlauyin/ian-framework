use std::fmt;
use std::time::Duration;

const DEFAULT_CAPACITY: usize = 10_000;
const DEFAULT_SHIP_EVERY: Duration = Duration::from_secs(5);
// log_request records the query and the JSON body, so a credential field must not reach a log.
pub const DEFAULT_MASK_PATTERNS: [&str; 7] = [
    r"(?i)password",
    r"(?i)passwd",
    r"(?i)secret",
    r"(?i)token",
    r"(?i)authorization",
    r"(?i)api[_-]?key",
    r"(?i)cookie",
];

#[derive(Clone, PartialEq, Eq)]
pub struct ShipperConfig {
    pub(crate) app: String,
    pub(crate) endpoint: String,
    pub(crate) post_api_key: String,
    pub(crate) capacity: usize,
    pub(crate) mask_patterns: Vec<String>,
    pub(crate) ship_every: Duration,
}

impl ShipperConfig {
    pub fn new(
        app: impl Into<String>,
        endpoint: impl Into<String>,
        post_api_key: impl Into<String>,
        mask_patterns: Vec<String>,
    ) -> Self {
        Self {
            app: app.into(),
            endpoint: endpoint.into(),
            post_api_key: post_api_key.into(),
            capacity: DEFAULT_CAPACITY,
            mask_patterns,
            ship_every: DEFAULT_SHIP_EVERY,
        }
    }
}

impl fmt::Debug for ShipperConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShipperConfig")
            .field("app", &self.app)
            .field("endpoint", &self.endpoint)
            .field("post_api_key", &"***")
            .field("capacity", &self.capacity)
            .field("mask_patterns", &self.mask_patterns)
            .field("ship_every", &self.ship_every)
            .finish()
    }
}
