use ian_framework_rs::exception;
use ian_framework_rs::gcp::storage::client::*;
use ian_framework_rs::logging::LogLevel;

#[test]
fn only_a_missing_object_becomes_none() {
    let missing = Err::<u8, _>(exception!(code = "RESOURCE_NOT_FOUND", "gone"));
    assert_eq!(none_if_not_found(missing).unwrap(), None);
    let other = Err::<u8, _>(exception!(code = "API_FAILED", "bad"));
    assert!(none_if_not_found(other).is_err());
    assert_eq!(none_if_not_found(Ok(1_u8)).unwrap(), Some(1));
}

#[test]
fn a_missing_object_writes_no_operations_record() {
    let missing = exception!(code = "RESOURCE_NOT_FOUND", "gone");
    assert!(operation_log_event("read", "notes.txt", "my-bucket", &missing).is_none());
}

#[test]
fn a_failed_operation_writes_an_error_record() {
    let err = exception!(code = "API_FAILED", "put object failed");
    let event = operation_log_event("write", "notes.txt", "my-bucket", &err).unwrap();
    assert_eq!(event.action, "GCS/write:/notes.txt");
    assert_eq!(event.level, LogLevel::Error);
    assert_eq!(event.context.len(), 0);
    assert_eq!(event.extra.len(), 1);
    assert_eq!(event.extra["bucket"], "my-bucket");
    let message = event.error_message.unwrap();
    assert!(message.contains("API_FAILED"));
    assert!(message.contains("put object failed"));
}
