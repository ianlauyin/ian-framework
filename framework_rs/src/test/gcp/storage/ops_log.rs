use crate::gcp::storage::ops_log::*;
use crate::logging::{LogLevel, LogValue};

#[test]
fn a_record_names_the_operation_and_the_bucket() {
    let event = OpsRecord {
        operation: "stat",
        object: "a/b.txt",
        latency_ms: 5,
        error: "boom",
    }
    .into_event("my-bucket");
    assert_eq!(event.action, "GCS/stat:/a/b.txt");
    assert_eq!(event.level, LogLevel::Warn);
    assert_eq!(event.latency_ms, 5);
    assert_eq!(
        event.extra.get("bucket"),
        Some(&LogValue::from("my-bucket"))
    );
    assert_eq!(event.extra.get("error"), Some(&LogValue::from("boom")));
}
