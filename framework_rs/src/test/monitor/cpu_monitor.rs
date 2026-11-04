use std::time::Duration;

use crate::shutdown::ShutdownReceiver;

use crate::logging::LogLevel;
use crate::monitor::cpu_monitor::*;

use serde_json::json;

fn sample(idle: u64, total: u64) -> CpuSample {
    CpuSample { idle, total }
}

#[test]
fn config_defaults() {
    let config = CpuMonitorConfig::new(80.0);
    assert_eq!(config.threshold_percent, 80.0);
    assert_eq!(config.interval, Duration::from_secs(30));
}

#[cfg(not(target_os = "linux"))]
#[test]
fn new_returns_none_when_the_os_is_not_linux() {
    assert!(CpuMonitor::new(CpuMonitorConfig::new(80.0)).is_none());
}

#[cfg(target_os = "linux")]
#[test]
fn new_rejects_a_bad_threshold_or_interval() {
    for threshold in [0.0, -1.0, 100.1, f64::NAN] {
        assert!(CpuMonitor::new(CpuMonitorConfig::new(threshold)).is_none());
    }
    for threshold in [0.1, 80.0, 100.0] {
        assert!(CpuMonitor::new(CpuMonitorConfig::new(threshold)).is_some());
    }
    let mut config = CpuMonitorConfig::new(80.0);
    config.interval = Duration::ZERO;
    assert!(CpuMonitor::new(config).is_none());
}

#[test]
fn parse_sample_reads_the_total_cpu_line() {
    let stat = "cpu  100 10 50 800 40 5 5 0 0 0\ncpu0 50 5 25 400 20 2 3 0 0 0\nintr 1\n";
    assert_eq!(parse_sample(stat), Some(sample(840, 1010)));
}

#[test]
fn parse_sample_rejects_bad_text() {
    for stat in ["", "intr 1 2 3", "cpu  1 2 3", "cpu  a b c d e f g h"] {
        assert_eq!(parse_sample(stat), None, "{stat:?}");
    }
}

#[test]
fn usage_percent_uses_the_change_between_samples() {
    assert_eq!(usage_percent(sample(0, 0), sample(25, 100)), Some(75.0));
    assert_eq!(usage_percent(sample(10, 100), sample(110, 200)), Some(0.0));
    assert_eq!(usage_percent(sample(10, 100), sample(10, 100)), None);
    assert_eq!(usage_percent(sample(10, 100), sample(5, 50)), None);
    assert_eq!(usage_percent(sample(0, 0), sample(50, 40)), None);
}

#[test]
fn parse_sample_rejects_a_total_that_overflows() {
    let stat = format!("cpu  {} 1 0 0 0 0 0 0", u64::MAX);
    assert_eq!(parse_sample(&stat), None);
}

#[test]
fn alert_gives_one_event_for_each_rise() {
    let mut alert = Alert::new(80.0);
    assert!(alert.check(79.9).is_none());
    let event = alert.check(85.26).unwrap();
    assert_eq!(event.action, "CPU/meetsThreshold");
    assert_eq!(event.level, LogLevel::Warn);
    assert_eq!(event.extra["cpu_percent"], json!(85.3));
    assert_eq!(event.extra["threshold_percent"], json!(80.0));
    assert!(alert.check(95.0).is_none());
    assert!(alert.check(80.0).is_none());
    assert!(alert.check(10.0).is_none());
    assert!(alert.check(80.0).is_some());
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn spawn_stops_on_shutdown() {
    let shutdown = ShutdownReceiver::new();
    let mut config = CpuMonitorConfig::new(100.0);
    config.interval = Duration::from_millis(10);
    let handle = CpuMonitor::new(config)
        .unwrap()
        .spawn(shutdown.clone(), |_| {});
    shutdown.trigger();
    tokio::time::timeout(Duration::from_secs(2), handle)
        .await
        .unwrap()
        .unwrap();
}

fn sampler(samples: Vec<Option<CpuSample>>) -> impl FnMut() -> Option<CpuSample> {
    let mut samples = samples.into_iter();
    move || samples.next().flatten()
}

#[tokio::test]
async fn run_sends_one_event_for_a_busy_cpu_and_skips_a_failed_read() {
    let shutdown = ShutdownReceiver::new();
    let mut config = CpuMonitorConfig::new(50.0);
    config.interval = Duration::from_millis(5);
    let monitor = CpuMonitor { config };
    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink_events = events.clone();
    let samples = vec![
        Some(sample(0, 0)),
        None,
        Some(sample(0, 100)),
        Some(sample(0, 200)),
    ];
    let run = monitor.run(
        shutdown.clone(),
        move |event| sink_events.lock().unwrap().push(event),
        sampler(samples),
    );
    let task = tokio::spawn(run);
    tokio::time::sleep(Duration::from_millis(100)).await;
    shutdown.trigger();
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(events.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn run_stops_when_the_first_read_fails() {
    let shutdown = ShutdownReceiver::new();
    let monitor = CpuMonitor {
        config: CpuMonitorConfig::new(50.0),
    };
    let run = monitor.run(shutdown, |_| {}, sampler(vec![None]));
    tokio::time::timeout(Duration::from_secs(2), run)
        .await
        .unwrap();
}
