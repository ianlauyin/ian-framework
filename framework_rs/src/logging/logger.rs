use std::collections::BTreeMap;
use std::sync::OnceLock;

use super::{LogEvent, LogLevel, LogShipper, LogValue};

static LOGGER: OnceLock<LogShipper> = OnceLock::new();

pub struct Logger;

impl Logger {
    pub fn init(shipper: LogShipper) -> Result<(), LogShipper> {
        LOGGER.set(shipper)
    }

    pub fn get() -> Option<&'static LogShipper> {
        LOGGER.get()
    }

    pub fn info(action: impl Into<String>, context: BTreeMap<String, LogValue>) {
        Self::log(LogEvent::info(action), context);
    }

    pub fn warn(action: impl Into<String>, context: BTreeMap<String, LogValue>) {
        Self::log(LogEvent::warn(action), context);
    }

    pub fn warn_with(
        action: impl Into<String>,
        error_message: impl Into<String>,
        context: BTreeMap<String, LogValue>,
    ) {
        Self::log(LogEvent::warn_with(action, error_message), context);
    }

    pub fn error(
        action: impl Into<String>,
        error_message: impl Into<String>,
        context: BTreeMap<String, LogValue>,
    ) {
        Self::log(LogEvent::error(action, error_message), context);
    }

    fn log(mut log_event: LogEvent, context: BTreeMap<String, LogValue>) {
        log_event.context = context;
        match LOGGER.get() {
            Some(shipper) => shipper.record(log_event),
            None => trace_event(&log_event),
        }
    }
}

fn trace_event(log_event: &LogEvent) {
    let context = serde_json::to_string(&log_event.context).unwrap_or_default();
    match log_event.level {
        LogLevel::Info => tracing::info!(action = %log_event.action, context = %context),
        LogLevel::Warn => tracing::warn!(
            action = %log_event.action,
            error_message = ?log_event.error_message,
            context = %context
        ),
        LogLevel::Error => tracing::error!(
            action = %log_event.action,
            error_message = ?log_event.error_message,
            context = %context
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_without_a_shipper_and_keeps_the_first_shipper() {
        Logger::error("TEST/error", "failed", BTreeMap::new());
        if Logger::get().is_none() {
            let first = LogShipper::new(crate::logging::ShipperConfig::new("my-app")).unwrap();
            assert!(Logger::init(first).is_ok());
        }
        let second = LogShipper::new(crate::logging::ShipperConfig::new("my-app")).unwrap();
        assert!(Logger::init(second).is_err());
        Logger::warn("TEST/warn", BTreeMap::new());
        Logger::warn_with("TEST/warn", "slow", BTreeMap::new());
    }
}
