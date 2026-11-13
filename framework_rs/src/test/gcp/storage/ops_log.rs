use crate::gcp::storage::ops_log::*;
use crate::logging::{LogLevel, LogValue};

#[test]
fn a_success_is_an_info_record() {
    let event = OpsRecord {
        operation: "stat",
        object: "a/b.txt",
        latency_ms: 5,
        error: None,
    }
    .into_event("my-bucket");
    assert_eq!(event.action, "GCS/stat:/a/b.txt");
    assert_eq!(event.level, LogLevel::Info);
    assert_eq!(event.error_message(), None);
    assert_eq!(event.latency_ms, 5);
    assert_eq!(
        event.extra.get("bucket"),
        Some(&LogValue::from("my-bucket"))
    );
    assert_eq!(
        event.extra.get("message"),
        Some(&LogValue::from("gcs operation succeeded"))
    );
}

#[test]
fn a_failure_is_an_error_record_with_the_error_message() {
    let event = OpsRecord {
        operation: "put",
        object: "a/b.txt",
        latency_ms: 7,
        error: Some("boom"),
    }
    .into_event("my-bucket");
    assert_eq!(event.level, LogLevel::Error);
    assert_eq!(event.error_message(), Some("boom"));
    assert_eq!(
        event.extra.get("message"),
        Some(&LogValue::from("gcs operation failed"))
    );
}
