use std::collections::BTreeMap;
use std::time::Duration;

use crate::exception;
use crate::exception::Exception;
use crate::logging::{LogEvent, Logger};
use crate::monitor::alert::Alert;
use crate::shutdown::ShutdownReceiver;

const STAT_PATH: &str = "/proc/stat";
const ACTION: &str = "CPU/meetsThreshold";

pub const CPU_STAT_INVALID: &str = "CPU_STAT_INVALID";
pub const CPU_USAGE_INVALID: &str = "CPU_USAGE_INVALID";

#[derive(Debug, Clone)]
pub struct CpuMonitor {
    pub(crate) threshold_percent: f64,
    pub(crate) interval: Duration,
}

impl CpuMonitor {
    pub fn new(threshold_percent: f64, interval: Duration) -> Option<Self> {
        if !(threshold_percent > 0.0 && threshold_percent <= 100.0) || interval.is_zero() {
            Logger::warn_with(
                "CPU/invalidConfig",
                BTreeMap::from([
                    ("threshold_percent".to_string(), threshold_percent.into()),
                    ("interval_seconds".to_string(), interval.as_secs_f64().into()),
                ]),
                "the threshold must be more than 0 and at most 100, and the interval must not be zero"
                    .into(),
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
        tokio::spawn(self.run(shutdown, sink, move || {
            parse_sample(&std::fs::read_to_string(STAT_PATH).ok()?).ok()
        }))
    }

    pub(crate) async fn run<F, S>(self, shutdown: ShutdownReceiver, sink: F, mut sample: S)
    where
        F: Fn(LogEvent),
        S: FnMut() -> Option<CpuSample>,
    {
        let Some(mut previous) = sample() else {
            Logger::error(
                "CPU/cannotReadStat",
                BTreeMap::from([("path".to_string(), STAT_PATH.into())]),
                "cannot read the cpu stat file".into(),
            );
            return;
        };
        let mut ticker = tokio::time::interval(self.interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        ticker.tick().await;
        let stop = shutdown.wait();
        tokio::pin!(stop);
        let mut alert = Alert::new(ACTION, self.threshold_percent);
        let mut read_failed = false;
        loop {
            tokio::select! {
                biased;
                _ = &mut stop => break,
                _ = ticker.tick() => {}
            }
            let Some(current) = sample() else {
                if !read_failed {
                    read_failed = true;
                    Logger::error(
                        "CPU/cannotReadStat",
                        BTreeMap::from([("path".to_string(), STAT_PATH.into())]),
                        "cannot read the cpu stat file".into(),
                    );
                }
                continue;
            };
            read_failed = false;
            if let Ok(percent) = usage_percent(previous, current) {
                if let Some(event) = alert.check(percent) {
                    sink(event);
                }
            }
            previous = current;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CpuSample {
    pub(crate) idle: u64,
    pub(crate) total: u64,
}

pub(crate) fn parse_sample(stat: &str) -> Result<CpuSample, Exception> {
    let Some(line) = stat.lines().next() else {
        return Err(exception!(code = CPU_STAT_INVALID, "cpu stat is not valid"));
    };
    let mut fields = line.split_whitespace();
    if fields.next() != Some("cpu") {
        return Err(exception!(code = CPU_STAT_INVALID, "cpu stat is not valid"));
    }
    let Some(values) = fields
        .take(8)
        .map(|field| field.parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()
    else {
        return Err(exception!(code = CPU_STAT_INVALID, "cpu stat is not valid"));
    };
    let (Some(idle_field), Some(wait_field)) = (values.get(3), values.get(4)) else {
        return Err(exception!(code = CPU_STAT_INVALID, "cpu stat is not valid"));
    };
    let Some(idle) = idle_field.checked_add(*wait_field) else {
        return Err(exception!(code = CPU_STAT_INVALID, "cpu stat is not valid"));
    };
    let Some(total) = values
        .iter()
        .try_fold(0u64, |sum, value| sum.checked_add(*value))
    else {
        return Err(exception!(code = CPU_STAT_INVALID, "cpu stat is not valid"));
    };
    Ok(CpuSample { idle, total })
}

pub(crate) fn usage_percent(previous: CpuSample, current: CpuSample) -> Result<f64, Exception> {
    let Some(total) = current.total.checked_sub(previous.total) else {
        return Err(exception!(
            code = CPU_USAGE_INVALID,
            "cpu usage is not valid"
        ));
    };
    let Some(idle) = current.idle.checked_sub(previous.idle) else {
        return Err(exception!(
            code = CPU_USAGE_INVALID,
            "cpu usage is not valid"
        ));
    };
    if total == 0 || idle > total {
        return Err(exception!(
            code = CPU_USAGE_INVALID,
            "cpu usage is not valid"
        ));
    }
    Ok(100.0 * (total - idle) as f64 / total as f64)
}
