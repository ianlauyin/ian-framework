mod client_log;
mod event;
mod log_batch;
mod log_shipper;
mod logger;
mod shipper_config;

use std::collections::BTreeMap;

use serde_json::Map;

use crate::time::hkt_now;

pub use client_log::client_log_router;
pub use event::{LogEvent, LogLevel, LogValue};
pub use log_batch::LogBatch;
pub use log_shipper::LogShipper;
pub use logger::Logger;
pub use shipper_config::ShipperConfig;

const SHIP_CHUNK: usize = 1_000;

pub(crate) fn event(action: impl Into<String>, level: LogLevel) -> LogEvent {
    LogEvent {
        ts: hkt_now(),
        id: String::new(),
        action: action.into(),
        latency_ms: None,
        level,
        context: BTreeMap::new(),
        info: Map::new(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn buffered_events(shipper: &LogShipper) -> Vec<LogEvent> {
        shipper.lock_buffer().events.iter().cloned().collect()
    }
}
