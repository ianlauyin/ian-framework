use std::path::PathBuf;
use std::time::Duration;

use serde_json::json;

use crate::logging::{event, LogEvent, LogLevel};
use crate::shutdown::ShutdownReceiver;

const DEFAULT_PATH: &str = "/";
const DEFAULT_INTERVAL: Duration = Duration::from_secs(60);
const ACTION: &str = "DISK/meets threshold";

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
                tracing::warn!(
                    "disk monitor cannot read the usage of {}, so it stops",
                    path.display()
                );
                return;
            }
            let mut ticker = tokio::time::interval(self.config.interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            ticker.tick().await;
            let stop = shutdown.wait();
            tokio::pin!(stop);
            let mut alert = Alert::new(self.config.threshold_percent, path.clone());
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

struct Alert {
    threshold_percent: f64,
    path: PathBuf,
    active: bool,
}

impl Alert {
    fn new(threshold_percent: f64, path: PathBuf) -> Self {
        Self {
            threshold_percent,
            path,
            active: false,
        }
    }

    // One event for each rise to the threshold. The alert arms again when the usage falls below it.
    fn check(&mut self, percent: f64) -> Option<LogEvent> {
        if percent < self.threshold_percent {
            self.active = false;
            return None;
        }
        if self.active {
            return None;
        }
        self.active = true;
        let mut event = event(ACTION, LogLevel::Warn);
        event
            .extra
            .insert("disk_percent".to_string(), json!(round_one(percent)));
        event.extra.insert(
            "threshold_percent".to_string(),
            json!(self.threshold_percent),
        );
        event
            .extra
            .insert("path".to_string(), json!(self.path.to_string_lossy()));
        Some(event)
    }
}

#[cfg(unix)]
fn read_usage(path: &std::path::Path) -> Option<f64> {
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

#[cfg(not(unix))]
fn read_usage(_path: &std::path::Path) -> Option<f64> {
    None
}

// Same as df: the blocks that root cannot use are not free, and the blocks that only root can use are not counted.
fn usage_percent(blocks: u64, free: u64, available: u64) -> Option<f64> {
    let used = blocks.checked_sub(free)?;
    let total = used.checked_add(available)?;
    if total == 0 {
        return None;
    }
    Some(100.0 * used as f64 / total as f64)
}

fn round_one(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults() {
        let config = DiskMonitorConfig::new(90.0);
        assert_eq!(config.threshold_percent, 90.0);
        assert_eq!(config.path, PathBuf::from("/"));
        assert_eq!(config.interval, Duration::from_secs(60));
    }

    #[test]
    fn new_rejects_a_bad_threshold_or_interval() {
        for threshold in [0.0, -1.0, 100.1, f64::NAN] {
            assert!(DiskMonitor::new(DiskMonitorConfig::new(threshold)).is_none());
        }
        for threshold in [0.1, 90.0, 100.0] {
            assert!(DiskMonitor::new(DiskMonitorConfig::new(threshold)).is_some());
        }
        let mut config = DiskMonitorConfig::new(90.0);
        config.interval = Duration::ZERO;
        assert!(DiskMonitor::new(config).is_none());
    }

    #[test]
    fn usage_percent_follows_df() {
        assert_eq!(usage_percent(100, 40, 40), Some(60.0));
        assert_eq!(usage_percent(100, 50, 25), Some(50.0 * 100.0 / 75.0));
        assert_eq!(usage_percent(100, 100, 0), None);
        assert_eq!(usage_percent(0, 0, 0), None);
        assert_eq!(usage_percent(10, 20, 0), None);
    }

    #[test]
    fn alert_gives_one_event_for_each_rise() {
        let mut alert = Alert::new(90.0, PathBuf::from("/data"));
        assert!(alert.check(89.9).is_none());
        let event = alert.check(91.26).unwrap();
        assert_eq!(event.action, "DISK/meets threshold");
        assert_eq!(event.level, LogLevel::Warn);
        assert_eq!(event.extra["disk_percent"], json!(91.3));
        assert_eq!(event.extra["threshold_percent"], json!(90.0));
        assert_eq!(event.extra["path"], json!("/data"));
        assert!(alert.check(95.0).is_none());
        assert!(alert.check(90.0).is_none());
        assert!(alert.check(10.0).is_none());
        assert!(alert.check(90.0).is_some());
    }

    #[cfg(unix)]
    #[test]
    fn read_usage_reads_an_existing_path() {
        let percent = read_usage(std::path::Path::new("/")).unwrap();
        assert!((0.0..=100.0).contains(&percent));
        assert!(read_usage(std::path::Path::new("/no/such/path")).is_none());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn spawn_stops_on_shutdown() {
        let shutdown = ShutdownReceiver::new();
        let mut config = DiskMonitorConfig::new(100.0);
        config.interval = Duration::from_millis(10);
        let handle = DiskMonitor::new(config)
            .unwrap()
            .spawn(shutdown.clone(), |_| {});
        shutdown.trigger();
        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .unwrap()
            .unwrap();
    }
}
