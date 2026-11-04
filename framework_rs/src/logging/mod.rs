mod client_log;
mod event;
mod log_shipper;
mod logger;
mod shipper_config;

pub use client_log::{client_events, client_log_router};
pub use event::{LogEvent, LogLevel, LogValue};
pub use log_shipper::LogShipper;
pub use logger::Logger;
pub use shipper_config::ShipperConfig;

const SHIP_CHUNK: usize = 1_000;

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn buffered_events(shipper: &LogShipper) -> Vec<LogEvent> {
        shipper.lock_buffer().events.iter().cloned().collect()
    }
}
