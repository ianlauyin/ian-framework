mod client_log;
mod event;
mod log_batch;
mod log_request;
mod log_shipper;

use std::collections::BTreeMap;

use chrono::Utc;

pub use client_log::client_log_router;
pub use event::{LogEvent, LogLevel};
pub use log_batch::LogBatch;
pub use log_request::log_request;
pub use log_shipper::{LogShipper, ShipperConfig};

const SHIP_CHUNK: usize = 1_000;

pub(crate) fn event(
    action: impl Into<String>,
    message: impl Into<String>,
    level: LogLevel,
) -> LogEvent {
    LogEvent {
        ts: Utc::now(),
        id: String::new(),
        ip: String::new(),
        user_agent: String::new(),
        action: action.into(),
        status: None,
        latency_ms: None,
        level,
        message: message.into(),
        extra: BTreeMap::new(),
    }
}
