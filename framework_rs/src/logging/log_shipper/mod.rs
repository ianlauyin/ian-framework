pub(crate) mod http_log_shipper;

pub(crate) use http_log_shipper::SHIP_CHUNK;
pub use http_log_shipper::{API_KEY_HEADER_INVALID, HTTP_CLIENT_BUILD_FAILED, HttpLogShipper};

use std::future::Future;

use crate::logging::LogEvent;

pub trait LogShipper: Send + Sync {
    fn ship<'a>(&'a self, events: &'a [LogEvent]) -> impl Future<Output = bool> + Send + 'a;
}
