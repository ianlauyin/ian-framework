use std::time::Duration;

use serde_json::json;

use crate::logging::{event, LogEvent, LogLevel};
use crate::shutdown::ShutdownReceiver;

const STAT_PATH: &str = "/proc/stat";
const DEFAULT_INTERVAL: Duration = Duration::from_secs(30);
const ACTION: &str = "CPU/meets threshold";

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
        tokio::spawn(async move {
            let Some(mut previous) = read_sample() else {
                tracing::warn!("cpu monitor cannot read {STAT_PATH}, so it stops");
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
                let Some(current) = read_sample() else {
                    continue;
                };
                if let Some(percent) = usage_percent(previous, current) {
                    if let Some(event) = alert.check(percent) {
                        sink(event);
                    }
                }
                previous = current;
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CpuSample {
    idle: u64,
    total: u64,
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
            .extra
            .insert("cpu_percent".to_string(), json!(round_one(percent)));
        event.extra.insert(
            "threshold_percent".to_string(),
            json!(self.threshold_percent),
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
    if values.len() < 5 {
        return None;
    }
    Some(CpuSample {
        idle: values[3] + values[4],
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
        assert_eq!(event.action, "CPU/meets threshold");
        assert_eq!(event.level, LogLevel::Warn);
        assert_eq!(event.extra["cpu_percent"], json!(85.3));
        assert_eq!(event.extra["threshold_percent"], json!(80.0));
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
}
