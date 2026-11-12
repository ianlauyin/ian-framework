use crate::logging::{LogEvent, LogLevel};

pub(crate) struct OpsRecord<'a> {
    pub(crate) operation: &'a str,
    pub(crate) object: &'a str,
    pub(crate) latency_ms: u64,
    pub(crate) error: &'a str,
}

impl OpsRecord<'_> {
    pub(crate) fn into_event(self, bucket: &str) -> LogEvent {
        let action = format!("GCS/{}:/{}", self.operation, self.object);
        let mut event = LogEvent::build(action, LogLevel::Warn, None);
        event.latency_ms = self.latency_ms;
        for (key, value) in [
            ("message", "gcs operation failed"),
            ("bucket", bucket),
            ("error", self.error),
        ] {
            event
                .extra
                .insert(key.to_string(), value.to_string().into());
        }
        event
    }
}
