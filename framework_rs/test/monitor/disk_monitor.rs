use std::time::Duration;

use crate::shutdown::ShutdownReceiver;

use crate::logging::LogLevel;
use crate::monitor::disk_monitor::*;

use serde_json::json;

const INTERVAL: Duration = Duration::from_secs(60);

#[test]
fn new_rejects_a_bad_threshold_or_interval() {
    for threshold in [0.0, -1.0, 100.1, f64::NAN] {
        assert!(DiskMonitor::new(threshold, INTERVAL).is_none());
    }
    for threshold in [0.1, 90.0, 100.0] {
        assert!(DiskMonitor::new(threshold, INTERVAL).is_some());
    }
    assert!(DiskMonitor::new(90.0, Duration::ZERO).is_none());
}

#[test]
fn read_usage_reads_the_root_file_system() {
    let percent = read_usage().unwrap();
    assert!((0.0..=100.0).contains(&percent));
}

#[tokio::test]
async fn spawn_stops_on_shutdown() {
    let shutdown = ShutdownReceiver::new();
    let handle = DiskMonitor::new(100.0, Duration::from_millis(10))
        .unwrap()
        .spawn(shutdown.clone(), |_| {});
    shutdown.trigger();
    tokio::time::timeout(Duration::from_secs(2), handle)
        .await
        .unwrap()
        .unwrap();
}
