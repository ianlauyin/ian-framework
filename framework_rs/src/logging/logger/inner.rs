use std::sync::Mutex;

use crate::logging::log_shipper::HttpLogShipper;
use crate::logging::logger::log_buffer::LogBuffer;
use crate::logging::mask::Masks;

pub struct Inner {
    pub shipper: HttpLogShipper,
    pub masks: Masks,
    pub buffer: Mutex<LogBuffer>,
    pub shipping: tokio::sync::Mutex<()>,
}
