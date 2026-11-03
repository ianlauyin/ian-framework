use std::collections::BTreeMap;
use std::time::Duration;

use crate::logging::{event, LogEvent, LogLevel, LogValue, Logger};
use crate::shutdown::ShutdownReceiver;

const STAT_PATH: &str = "/proc/stat";
const DEFAULT_INTERVAL: Duration = Duration::from_secs(30);
const ACTION: &str = "CPU/meetsThreshold";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpuMonitorConfig {
    pub threshold_percent: f64,
    pub interval: Duration,
}

impl CpuMonitorConfig {
    pub fn new(threshold_percent: f64) -> Self {
        Self {
            threshold_percent,
            interval: DEFAULT_INTERVAL,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CpuMonitor {
    config: CpuMonitorConfig,
}

impl CpuMonitor {
    pub fn new(config: CpuMonitorConfig) -> Option<Self> {
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
        tokio::spawn(self.run(shutdown, sink, read_sample))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CpuSample {
    idle: u64,
    total: u64,
}

impl CpuMonitor {
    async fn run<F, S>(self, shutdown: ShutdownReceiver, sink: F, mut sample: S)
    where
        F: Fn(LogEvent),
        S: FnMut() -> Option<CpuSample>,
    {
        let Some(mut previous) = sample() else {
            Logger::error(
                "CPU/cannotReadStat",
                BTreeMap::from([("path".to_string(), LogValue::from(STAT_PATH))]),
            );
            return;
        };
        let mut ticker = tokio::time::interval(self.config.interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        ticker.tick().await;
        let stop = shutdown.wait();
        tokio::pin!(stop);
        let mut alert = Alert::new(self.config.threshold_percent);
        loop {
            tokio::select! {
                biased;
                _ = &mut stop => break,
                _ = ticker.tick() => {}
            }
            let Some(current) = sample() else {
                continue;
            };
            if let Some(percent) = usage_percent(previous, current) {
                if let Some(event) = alert.check(percent) {
                    sink(event);
                }
            }
            previous = current;
        }
    }
}

struct Alert {
    threshold_percent: f64,
    active: bool,
}

impl Alert {
    fn new(threshold_percent: f64) -> Self {
        Self {
            threshold_percent,
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
            .info
            .insert("cpu_percent".to_string(), round_one(percent).into());
        event.info.insert(
            "threshold_percent".to_string(),
            self.threshold_percent.into(),
        );
        Some(event)
    }
}

fn read_sample() -> Option<CpuSample> {
    parse_sample(&std::fs::read_to_string(STAT_PATH).ok()?)
}

fn parse_sample(stat: &str) -> Option<CpuSample> {
    let line = stat.lines().next()?;
    let mut fields = line.split_whitespace();
    if fields.next()? != "cpu" {
        return None;
    }
    // user, nice, system, idle, iowait, irq, softirq, steal. The guest times are in user and nice.
    let values = fields
        .take(8)
        .map(|field| field.parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()?;
    let idle = values.get(3)?.checked_add(*values.get(4)?)?;
    Some(CpuSample {
        idle,
        total: values.iter().sum(),
    })
}

fn usage_percent(previous: CpuSample, current: CpuSample) -> Option<f64> {
    let total = current.total.checked_sub(previous.total)?;
    let idle = current.idle.checked_sub(previous.idle)?;
    if total == 0 {
        return None;
    }
    Some(100.0 * total.saturating_sub(idle) as f64 / total as f64)
}

fn round_one(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn alert_gives_one_event_for_each_rise() {
        let mut alert = Alert::new(80.0);
        assert!(alert.check(79.9).is_none());
        let event = alert.check(85.26).unwrap();
        assert_eq!(event.action, "CPU/meetsThreshold");
        assert_eq!(event.level, LogLevel::Warn);
        assert_eq!(event.info["cpu_percent"], json!(85.3));
        assert_eq!(event.info["threshold_percent"], json!(80.0));
        assert!(alert.check(95.0).is_none());
        assert!(alert.check(80.0).is_none());
        assert!(alert.check(10.0).is_none());
        assert!(alert.check(80.0).is_some());
    }

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
        let monitor = CpuMonitor::new(config).unwrap();
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
        let monitor = CpuMonitor::new(CpuMonitorConfig::new(50.0)).unwrap();
        let run = monitor.run(shutdown, |_| {}, sampler(vec![None]));
        tokio::time::timeout(Duration::from_secs(2), run)
            .await
            .unwrap();
    }
}
