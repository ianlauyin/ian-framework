mod client_log;
mod event;
mod log_shipper;
mod logger;

use crate::logging::*;

pub(crate) fn buffered_events(shipper: &LogShipper) -> Vec<LogEvent> {
    shipper.lock_buffer().events.iter().cloned().collect()
}
