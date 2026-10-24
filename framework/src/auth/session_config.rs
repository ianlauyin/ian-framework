#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionConfig {
    pub prefix: String,
    pub cookie_name: String,
    pub ttl_secs: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            prefix: "session:".to_string(),
            cookie_name: "session".to_string(),
            ttl_secs: 7 * 24 * 60 * 60,
        }
    }
}
