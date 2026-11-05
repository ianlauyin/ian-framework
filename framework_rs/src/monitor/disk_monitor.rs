use std::collections::BTreeMap;
use std::time::Duration;

use crate::logging::{LogEvent, Logger};
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
            Logger::warn_with(
                "DISK/invalidConfig",
                BTreeMap::new(),
                format!(
                    "invalid disk monitor config: threshold_percent={threshold_percent} must be in (0, 100] and interval={interval:?} must not be zero"
                ),
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
                Logger::warn_with(
                    "DISK/cannotReadUsage",
                    BTreeMap::new(),
                    "cannot read the disk usage".into(),
                );
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

pub(crate) fn read_usage() -> Option<f64> {
    let stat = rustix::fs::statvfs("/").ok()?;
    // Same as df: the blocks that root cannot use are not free, and the blocks that only root can use are not counted.
    let used = stat.f_blocks.checked_sub(stat.f_bfree)?;
    let total = used.checked_add(stat.f_bavail)?;
    if total == 0 {
        return None;
    }
    Some(100.0 * used as f64 / total as f64)
}
