use std::future::Future;
use std::time::Instant;

use crate::logging::LogEvent;
use crate::logging::record;

pub async fn with_log<T>(
    work: impl Future<Output = T>,
    to_event: impl FnOnce(&mut T) -> LogEvent,
) -> T {
    let started = Instant::now();
    let mut output = work.await;
    record(to_event(&mut output).with_latency(started.elapsed()));
    output
}
