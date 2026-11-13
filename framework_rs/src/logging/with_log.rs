use std::future::Future;
use std::time::Instant;

use crate::logging::{LogEvent, Logger};

// `to_event` makes the event from the output. The latency is set after it, so it cannot replace the latency.
pub async fn with_log<T>(
    work: impl Future<Output = T>,
    to_event: impl FnOnce(&mut T) -> LogEvent,
) -> T {
    let started = Instant::now();
    let mut output = work.await;
    if let Some(logger) = Logger::get() {
        let mut event = to_event(&mut output);
        event.latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        logger.record(event);
    }
    output
}
