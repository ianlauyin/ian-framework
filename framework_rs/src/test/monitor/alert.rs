use crate::logging::LogLevel;
use crate::monitor::alert::Alert;

use serde_json::json;

#[test]
fn alert_gives_one_event_for_each_rise() {
    let mut alert = Alert::new("CPU/meetsThreshold", "cpu_percent", 80.0);
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
