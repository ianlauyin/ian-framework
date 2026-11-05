use crate::logging::LogEvent;

pub(crate) struct Alert {
    action: &'static str,
    threshold_percent: f64,
    active: bool,
}

impl Alert {
    pub(crate) fn new(action: &'static str, threshold_percent: f64) -> Self {
        Self {
            action,
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
        let mut event = LogEvent::warn(self.action);
        event
            .extra
            .insert("percent".to_string(), round_one(percent).into());
        event.extra.insert(
            "threshold_percent".to_string(),
            self.threshold_percent.into(),
        );
        Some(event)
    }
}

fn round_one(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}
