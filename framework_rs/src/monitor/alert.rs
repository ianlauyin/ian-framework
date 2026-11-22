use crate::logging::LogEvent;

pub struct Alert {
    action: &'static str,
    threshold_percent: f64,
    active: bool,
}

impl Alert {
    pub fn new(action: &'static str, threshold_percent: f64) -> Self {
        Self {
            action,
            threshold_percent,
            active: false,
        }
    }

    pub fn check(&mut self, percent: f64) -> Option<LogEvent> {
        if percent < self.threshold_percent {
            self.active = false;
            return None;
        }
        if self.active {
            return None;
        }
        self.active = true;
        Some(
            LogEvent::warn(self.action)
                .with_extra("percent", round_one(percent))
                .with_extra("threshold_percent", self.threshold_percent),
        )
    }
}

fn round_one(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}
