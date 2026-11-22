use ian_framework_rs::logging::*;

use crate::logging::{buffered_events, global_logger};

#[test]
fn it_puts_the_masked_event_in_the_buffer_of_the_logger() {
    let logger = global_logger();
    record(
        LogEvent::warn_with("record/buffers", "slow")
            .with_context("path", "/x?token=abc")
            .with_extra("password", "hunter2"),
    );
    let events: Vec<_> = buffered_events(logger)
        .into_iter()
        .filter(|event| event.action == "record/buffers")
        .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].level, LogLevel::Warn);
    assert_eq!(events[0].error_message.as_deref(), Some("slow"));
    assert_eq!(events[0].context["path"], "/x?***");
    assert_eq!(events[0].extra["password"], "***");
}
