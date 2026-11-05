mod client_log;
mod event;
mod log_shipper;
mod logger;

use crate::logging::*;

pub(crate) fn http_shipper(app: &'static str, endpoint: &str) -> HttpLogShipper {
    HttpLogShipper::new(app, endpoint.to_string(), "APP_KEY")
}

pub(crate) fn default_shipper(app: &'static str) -> HttpLogShipper {
    http_shipper(app, "https://logs.example")
}

pub(crate) fn default_mask_patterns() -> Vec<String> {
    DEFAULT_MASK_PATTERNS
        .iter()
        .map(|pattern| pattern.to_string())
        .collect()
}

pub(crate) fn logger(shipper: HttpLogShipper) -> Logger {
    Logger::new(shipper, default_mask_patterns())
}

pub(crate) fn client_log_event(event: &LogEvent) -> ClientLogEvent {
    serde_json::from_value(serde_json::to_value(event).unwrap()).unwrap()
}

pub(crate) fn assert_uuid_v7(id: &str) {
    let uuid = uuid::Uuid::parse_str(id).unwrap();
    assert_eq!(uuid.get_version_num(), 7);
}

pub(crate) fn buffered_events(logger: &Logger) -> Vec<LogEvent> {
    logger.lock_buffer().events.iter().cloned().collect()
}
