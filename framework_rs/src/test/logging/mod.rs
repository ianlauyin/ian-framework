mod client_log;
mod event;
mod log_shipper;
mod logger;

use crate::logging::*;

pub(crate) fn shipper_config(app: &str) -> ShipperConfig {
    ShipperConfig::new(
        app,
        "https://logs.example",
        "APP_KEY",
        DEFAULT_MASK_PATTERNS
            .iter()
            .map(|pattern| pattern.to_string())
            .collect(),
    )
}

pub(crate) fn buffered_events(shipper: &LogShipper) -> Vec<LogEvent> {
    shipper.lock_buffer().events.iter().cloned().collect()
}
