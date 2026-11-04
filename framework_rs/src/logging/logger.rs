use std::collections::HashMap;
use std::error::Error;
use std::sync::OnceLock;

use crate::exception::Exception;

use super::{LogEvent, LogLevel, LogShipper};

static LOGGER: OnceLock<LogShipper> = OnceLock::new();

pub struct Logger;

impl Logger {
    pub fn init(shipper: LogShipper) -> Result<(), LogShipper> {
        LOGGER.set(shipper)
    }

    pub fn get() -> Option<&'static LogShipper> {
        LOGGER.get()
    }

    pub fn info(action: impl Into<String>, context: HashMap<String, String>) {
        Self::log(LogEvent::info(action), context);
    }

    pub fn warn(action: impl Into<String>, context: HashMap<String, String>) {
        Self::log(LogEvent::warn(action), context);
    }

    pub fn warn_with(
        action: impl Into<String>,
        context: HashMap<String, String>,
        error: Box<dyn Error>,
    ) {
        Self::log(LogEvent::warn_with(action, error.to_string()), context);
    }

    pub fn error(
        action: impl Into<String>,
        context: HashMap<String, String>,
        error: Box<dyn Error>,
    ) {
        let error_message = match error.downcast::<Exception>() {
            Ok(exception) => {
                let message = exception.to_string();
                let trace = String::from(exception.as_ref());
                format!("{message}\n{trace}")
            }
            Err(error) => error.to_string(),
        };
        Self::log(LogEvent::error(action, error_message), context);
    }

    fn log(mut log_event: LogEvent, context: HashMap<String, String>) {
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
