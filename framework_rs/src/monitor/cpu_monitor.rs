use std::collections::HashMap;
use std::time::Duration;

use crate::logging::{LogEvent, Logger};
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
pub(crate) struct CpuSample {
    pub(crate) idle: u64,
    pub(crate) total: u64,
}

impl CpuMonitor {
    pub(crate) async fn run<F, S>(self, shutdown: ShutdownReceiver, sink: F, mut sample: S)
    where
        F: Fn(LogEvent),
        S: FnMut() -> Option<CpuSample>,
    {
        let Some(mut previous) = sample() else {
            Logger::error(
                "CPU/cannotReadStat",
                HashMap::from([("path".to_string(), STAT_PATH.to_string())]),
                "cannot read the cpu stat file".into(),
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

pub(crate) struct Alert {
    threshold_percent: f64,
    active: bool,
}

impl Alert {
    pub(crate) fn new(threshold_percent: f64) -> Self {
        Self {
            threshold_percent,
            active: false,
        }
    }

    // One event for each rise to the threshold. The alert arms again when the usage falls below it.
    pub(crate) fn check(&mut self, percent: f64) -> Option<LogEvent> {
        if percent < self.threshold_percent {
            self.active = false;
            return None;
        }
        if self.active {
            return None;
        }
        self.active = true;
        let mut event = LogEvent::warn(ACTION);
        event
            .extra
            .insert("cpu_percent".to_string(), round_one(percent).to_string());
        event.extra.insert(
            "threshold_percent".to_string(),
            self.threshold_percent.to_string(),
        );
        Some(event)
    }
}

fn read_sample() -> Option<CpuSample> {
    parse_sample(&std::fs::read_to_string(STAT_PATH).ok()?)
}

pub(crate) fn parse_sample(stat: &str) -> Option<CpuSample> {
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

pub(crate) fn usage_percent(previous: CpuSample, current: CpuSample) -> Option<f64> {
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
