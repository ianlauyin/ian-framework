use crate::logging::LogLevel;

pub(super) struct OpsRecord<'a> {
    pub(super) level: LogLevel,
    pub(super) message: &'a str,
    pub(super) method: &'a str,
    pub(super) url: &'a str,
    pub(super) status: Option<u16>,
    pub(super) latency_ms: Option<u64>,
    pub(super) extra: &'a [(&'a str, &'a str)],
}
