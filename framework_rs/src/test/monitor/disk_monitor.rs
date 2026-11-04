use std::path::PathBuf;
use std::time::Duration;

use crate::shutdown::ShutdownReceiver;

use crate::logging::LogLevel;
use crate::monitor::disk_monitor::*;

use serde_json::json;

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
    assert_eq!(event.action, "DISK/meetsThreshold");
    assert_eq!(event.level, LogLevel::Warn);
    assert_eq!(event.info["disk_percent"], json!(91.3));
    assert_eq!(event.info["threshold_percent"], json!(90.0));
    assert_eq!(event.info["path"], json!("/data"));
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
