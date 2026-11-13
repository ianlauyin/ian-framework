use crate::logging::{LogEvent, LogLevel};

pub(crate) struct OpsRecord<'a> {
    pub(crate) operation: &'a str,
    pub(crate) object: &'a str,
    pub(crate) latency_ms: u64,
    // `None` is a success. `Some` is the log text of the failure.
    pub(crate) error: Option<&'a str>,
}

impl OpsRecord<'_> {
    pub(crate) fn into_event(self, bucket: &str) -> LogEvent {
        let action = format!("GCS/{}:/{}", self.operation, self.object);
        let (level, message) = match self.error {
            None => (LogLevel::Info, "gcs operation succeeded"),
            Some(_) => (LogLevel::Error, "gcs operation failed"),
        };
        let mut event = LogEvent::build(action, level, self.error.map(str::to_string));
        event.latency_ms = self.latency_ms;
        for (key, value) in [("message", message), ("bucket", bucket)] {
            event
                .extra
                .insert(key.to_string(), value.to_string().into());
        }
        event
    }
}
