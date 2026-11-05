pub(crate) mod http_log_shipper;

use std::future::Future;

use crate::logging::LogEvent;

pub use http_log_shipper::HttpLogShipper;

pub(crate) const SHIP_CHUNK: usize = 1_000;

pub trait LogShipper: Send + Sync {
    fn ship<'a>(&'a self, events: &'a [LogEvent]) -> impl Future<Output = bool> + Send + 'a;
}
