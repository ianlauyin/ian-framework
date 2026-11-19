use crate::logging::{LogEvent, LogLevel};

pub(crate) fn trace_event(log_event: &LogEvent) {
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
