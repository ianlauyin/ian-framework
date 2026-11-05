use std::time::Duration;

use crate::shutdown::ShutdownReceiver;

use crate::logging::LogLevel;
use crate::monitor::cpu_monitor::*;

use serde_json::json;

const INTERVAL: Duration = Duration::from_secs(30);

fn sample(idle: u64, total: u64) -> CpuSample {
    CpuSample { idle, total }
}

#[test]
fn new_rejects_a_bad_threshold_or_interval() {
    for threshold in [0.0, -1.0, 100.1, f64::NAN] {
        assert!(CpuMonitor::new(threshold, INTERVAL).is_none());
    }
    for threshold in [0.1, 80.0, 100.0] {
        assert!(CpuMonitor::new(threshold, INTERVAL).is_some());
    }
    assert!(CpuMonitor::new(80.0, Duration::ZERO).is_none());
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

#[tokio::test]
async fn spawn_stops_on_shutdown() {
    let shutdown = ShutdownReceiver::new();
    let handle = CpuMonitor::new(100.0, Duration::from_millis(10))
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
    let monitor = CpuMonitor::new(50.0, Duration::from_millis(5)).unwrap();
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
    let monitor = CpuMonitor::new(50.0, INTERVAL).unwrap();
    let run = monitor.run(shutdown, |_| {}, sampler(vec![None]));
    tokio::time::timeout(Duration::from_secs(2), run)
        .await
        .unwrap();
}
