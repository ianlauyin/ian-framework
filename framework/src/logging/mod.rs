mod event;
mod middleware;
mod shipper;

pub use event::{event, parse_json_line, parse_jsonl, to_json_line, to_jsonl, LogEvent, LogLevel};
pub use middleware::log_request;
pub use shipper::{
    buffered_events, buffered_len, dropped, endpoint, log_shipper, minimum_level, project, record,
    service, ship_batch, shipper_config, spawn_ship_loop, LogShipper, ShipMode, ShipResult,
    ShipperConfig, DEFAULT_BATCH_SIZE, DEFAULT_CAPACITY, DEFAULT_SHIP_EVERY, MAX_MESSAGE_BYTES,
    MAX_PATH_BYTES, MAX_REQUEST_ID_BYTES, SHIP_TIMEOUT,
};

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogErrorKind {
    Config,
    Transport,
    Api,
    Encode,
}

pub struct LogError {
    kind: LogErrorKind,
    status: Option<u16>,
    message: String,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl LogError {
    pub fn kind(&self) -> LogErrorKind {
        self.kind
    }

    pub(crate) fn new(kind: LogErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            status: None,
            message: message.into(),
            source: None,
        }
    }

    pub(crate) fn with_status(kind: LogErrorKind, status: u16, message: impl Into<String>) -> Self {
        Self {
            kind,
            status: Some(status),
            message: message.into(),
            source: None,
        }
    }

    pub(crate) fn with_source(
        kind: LogErrorKind,
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            status: None,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
}

impl fmt::Debug for LogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LogError")
            .field("kind", &self.kind)
            .field("status", &self.status)
            .field("message", &self.message)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for LogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for LogError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.source {
            Some(source) => Some(source.as_ref()),
            None => None,
        }
    }
}

pub(super) fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
