use std::time::Duration;

pub const SHIP_EVERY: Duration = Duration::from_secs(60);
pub const FLUSH_RETRY_EVERY: Duration = Duration::from_secs(1);
pub const FLUSH_RETRY_LIMIT: u8 = 3;
