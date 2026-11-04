pub(crate) mod client_log;
pub(crate) mod event;
pub(crate) mod log_shipper;
pub(crate) mod logger;
pub(crate) mod shipper_config;

pub use client_log::{client_events, client_log_router};
pub use event::{LogEvent, LogLevel};
pub use log_shipper::LogShipper;
pub use logger::Logger;
pub use shipper_config::ShipperConfig;

pub(crate) const SHIP_CHUNK: usize = 1_000;
