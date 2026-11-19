use std::sync::Mutex;

use crate::logging::log_shipper::HttpLogShipper;
use crate::logging::logger::log_buffer::LogBuffer;
use crate::logging::mask::Masks;

pub(crate) struct Inner {
    pub(crate) shipper: HttpLogShipper,
    pub(crate) masks: Masks,
    pub(crate) buffer: Mutex<LogBuffer>,
    pub(crate) shipping: tokio::sync::Mutex<()>,
}
