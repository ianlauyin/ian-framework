use crate::logging::logger::trace_event::trace_event;
use crate::logging::{LogEvent, Logger};

pub fn record(event: LogEvent) {
    match Logger::get() {
        Some(logger) => logger.record(event),
        None => trace_event(&event),
    }
}
