use std::collections::BTreeMap;
use std::sync::OnceLock;

use super::{event, LogLevel, LogShipper, LogValue};

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
        Self::log(LogLevel::Info, action.into(), context);
    }

    pub fn warn(action: impl Into<String>, context: BTreeMap<String, LogValue>) {
        Self::log(LogLevel::Warn, action.into(), context);
    }

    pub fn error(action: impl Into<String>, context: BTreeMap<String, LogValue>) {
        Self::log(LogLevel::Error, action.into(), context);
    }

    fn log(level: LogLevel, action: String, context: BTreeMap<String, LogValue>) {
        let mut log_event = event(action, level);
        log_event.context = context;
        match LOGGER.get() {
            Some(shipper) => shipper.record(log_event),
            None => trace_event(&log_event),
        }
    }
}

fn trace_event(log_event: &super::LogEvent) {
    let context = serde_json::to_string(&log_event.context).unwrap_or_default();
    match log_event.level {
        LogLevel::Info => tracing::info!(action = %log_event.action, context = %context),
        LogLevel::Warn => tracing::warn!(action = %log_event.action, context = %context),
        LogLevel::Error => tracing::error!(action = %log_event.action, context = %context),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_without_a_shipper_and_keeps_the_first_shipper() {
        Logger::error("TEST/error", BTreeMap::new());
        if Logger::get().is_none() {
            let first = LogShipper::new(crate::logging::ShipperConfig::new("my-app")).unwrap();
            assert!(Logger::init(first).is_ok());
        }
        let second = LogShipper::new(crate::logging::ShipperConfig::new("my-app")).unwrap();
        assert!(Logger::init(second).is_err());
        Logger::warn("TEST/warn", BTreeMap::new());
    }
}
