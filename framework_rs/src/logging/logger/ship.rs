use std::time::Duration;

pub(crate) const SHIP_EVERY: Duration = Duration::from_secs(60);
pub(crate) const FLUSH_RETRY_EVERY: Duration = Duration::from_secs(1);
pub(crate) const FLUSH_RETRY_LIMIT: u8 = 3;
