use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::logging::{LogEvent, Logger};
use crate::monitor::alert::Alert;
use crate::shutdown::ShutdownReceiver;

const DEFAULT_PATH: &str = "/";
const DEFAULT_INTERVAL: Duration = Duration::from_secs(60);
const ACTION: &str = "DISK/meetsThreshold";

#[derive(Debug, Clone, PartialEq)]
pub struct DiskMonitorConfig {
    pub threshold_percent: f64,
    pub path: PathBuf,
    pub interval: Duration,
}

impl DiskMonitorConfig {
    pub fn new(threshold_percent: f64) -> Self {
        Self {
            threshold_percent,
            path: PathBuf::from(DEFAULT_PATH),
            interval: DEFAULT_INTERVAL,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiskMonitor {
    config: DiskMonitorConfig,
}

impl DiskMonitor {
    pub fn new(config: DiskMonitorConfig) -> Option<Self> {
        let threshold = config.threshold_percent;
        if !(threshold > 0.0 && threshold <= 100.0) || config.interval.is_zero() {
            return None;
        }
        Some(Self { config })
    }

    pub fn spawn<F>(self, shutdown: ShutdownReceiver, sink: F) -> tokio::task::JoinHandle<()>
    where
        F: Fn(LogEvent) + Send + 'static,
    {
        tokio::spawn(async move {
            let path = self.config.path;
            if read_usage(&path).is_none() {
                Logger::warn_with(
                    "DISK/cannotReadUsage",
                    BTreeMap::from([("path".to_string(), path.display().to_string().into())]),
                    "cannot read the disk usage".into(),
                );
                return;
            }
            let mut ticker = tokio::time::interval(self.config.interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            ticker.tick().await;
            let stop = shutdown.wait();
            tokio::pin!(stop);
            let mut alert = Alert::new(ACTION, "disk_percent", self.config.threshold_percent);
            loop {
                if let Some(percent) = read_usage(&path) {
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

pub(crate) fn read_usage(path: &std::path::Path) -> Option<f64> {
    use std::os::unix::ffi::OsStrExt;

    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: path is a valid C string and stat is a valid buffer. stat is read only when the call succeeds.
    let stat = unsafe {
        if libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) != 0 {
            return None;
        }
        stat.assume_init()
    };
    usage_percent(
        stat.f_blocks as u64,
        stat.f_bfree as u64,
        stat.f_bavail as u64,
    )
}

// Same as df: the blocks that root cannot use are not free, and the blocks that only root can use are not counted.
pub(crate) fn usage_percent(blocks: u64, free: u64, available: u64) -> Option<f64> {
    let used = blocks.checked_sub(free)?;
    let total = used.checked_add(available)?;
    if total == 0 {
        return None;
    }
    Some(100.0 * used as f64 / total as f64)
}
