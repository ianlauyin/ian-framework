use std::future::Future;

use crate::exception::Exception;
use crate::logging::LogEvent;

pub mod http_log_shipper;

pub use http_log_shipper::SHIP_CHUNK;
pub use http_log_shipper::{
    API_KEY_HEADER_INVALID, HTTP_CLIENT_BUILD_FAILED, HttpLogShipper, LOG_ENCODE_FAILED,
    LOG_SHIP_FAILED,
};

pub trait LogShipper: Send + Sync {
    fn ship<'a>(
        &'a self,
        events: &'a [LogEvent],
    ) -> impl Future<Output = Result<(), Exception>> + Send + 'a;
}
