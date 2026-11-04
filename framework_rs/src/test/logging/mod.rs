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

pub(crate) fn client_log_event(event: &LogEvent) -> ClientLogEvent {
    serde_json::from_value(serde_json::to_value(event).unwrap()).unwrap()
}

pub(crate) fn assert_uuid_v7(id: &str) {
    let uuid = uuid::Uuid::parse_str(id).unwrap();
    assert_eq!(uuid.get_version_num(), 7);
}

pub(crate) fn buffered_events(shipper: &LogShipper) -> Vec<LogEvent> {
    shipper.lock_buffer().events.iter().cloned().collect()
}
