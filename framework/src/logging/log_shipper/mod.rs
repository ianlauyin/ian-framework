mod new;
mod record;
mod ship;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::LogEvent;

pub use new::ShipperConfig;

const DEFAULT_CAPACITY: usize = 10_000;
const SHIP_TIMEOUT: Duration = Duration::from_secs(10);
const CAPACITY_WARN_NOTE: &str = "capacity meets 80%";

#[derive(Clone)]
pub struct LogShipper {
    inner: Arc<Inner>,
}

impl LogShipper {
    fn lock_buffer(&self) -> std::sync::MutexGuard<'_, Buffer> {
        self.inner
            .buffer
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    fn buffered_len(&self) -> usize {
        self.lock_buffer().events.len()
    }

    #[cfg(test)]
    pub(crate) fn buffered_events(&self) -> Vec<LogEvent> {
        self.lock_buffer().events.iter().cloned().collect()
    }
}

impl std::fmt::Debug for LogShipper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogShipper")
            .field("app", &self.inner.app)
            .field("endpoint", &self.inner.endpoint)
            .field("buffered", &self.buffered_len())
            .finish_non_exhaustive()
    }
}

struct Buffer {
    events: VecDeque<LogEvent>,
    capacity_warned: bool,
}

struct Inner {
    app: String,
    endpoint: Option<String>,
    capacity: usize,
    buffer: Mutex<Buffer>,
    http: Option<reqwest::Client>,
    shipping: tokio::sync::Mutex<()>,
}

fn meets_capacity_warn(len: usize, capacity: usize) -> bool {
    (len as u128) * 5 >= (capacity as u128) * 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_warn_limit_is_80_percent() {
        assert!(!meets_capacity_warn(0, 1));
        assert!(meets_capacity_warn(1, 1));
        assert!(!meets_capacity_warn(3, 4));
        assert!(meets_capacity_warn(4, 4));
        assert!(!meets_capacity_warn(3, 5));
        assert!(meets_capacity_warn(4, 5));
        assert!(!meets_capacity_warn(7999, 10_000));
        assert!(meets_capacity_warn(8000, 10_000));
    }
}
