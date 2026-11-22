use std::time::Duration;

use crate::logging::LogEvent;
use crate::logging::record;
use crate::monitor::alert::Alert;
use crate::shutdown::ShutdownReceiver;

const ACTION: &str = "DISK/meetsThreshold";

#[derive(Debug, Clone)]
pub struct DiskMonitor {
    threshold_percent: f64,
    interval: Duration,
}

impl DiskMonitor {
    pub fn new(threshold_percent: f64, interval: Duration) -> Option<Self> {
        if !(threshold_percent > 0.0 && threshold_percent <= 100.0) || interval.is_zero() {
            record(
                LogEvent::warn_with(
                    "DISK/invalidConfig",
                    "the threshold must be more than 0 and at most 100, and the interval must not be zero",
                )
                .with_context("threshold_percent", threshold_percent)
                .with_context("interval_seconds", interval.as_secs_f64()),
            );
            return None;
        }
        Some(Self {
            threshold_percent,
            interval,
        })
    }

    pub fn spawn<F>(self, shutdown: ShutdownReceiver, sink: F) -> tokio::task::JoinHandle<()>
    where
        F: Fn(LogEvent) + Send + 'static,
    {
        tokio::spawn(async move {
            if read_usage().is_none() {
                record(LogEvent::error(
                    "DISK/cannotReadUsage",
                    "cannot read the disk usage",
                ));
                return;
            }
            let mut ticker = tokio::time::interval(self.interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            ticker.tick().await;
            let stop = shutdown.wait();
            tokio::pin!(stop);
            let mut alert = Alert::new(ACTION, self.threshold_percent);
            loop {
                if let Some(percent) = read_usage() {
                    if let Some(event) = alert.check(percent) {
                        sink(event);
                    }
                }
                tokio::select! {
                    biased;
                    _ = &mut stop => break,
                    _ = ticker.tick() => {}
                }
            }
        })
    }
}

pub fn read_usage() -> Option<f64> {
    let stat = rustix::fs::statvfs("/").ok()?;
    let used = stat.f_blocks.checked_sub(stat.f_bfree)?;
    let total = used.checked_add(stat.f_bavail)?;
    if total == 0 {
        return None;
    }
    Some(100.0 * used as f64 / total as f64)
}
