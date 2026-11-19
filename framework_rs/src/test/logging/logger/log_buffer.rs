use crate::logging::logger::log_buffer::{CAPACITY, CAPACITY_WARN_NOTE, LogBuffer};
use crate::logging::{LogEvent, LogLevel};

const LIMIT: usize = CAPACITY / 5 * 4;

fn fill(buffer: &mut LogBuffer, count: usize) {
    for index in 0..count {
        buffer.push(LogEvent::info(index.to_string()));
    }
}

fn warn_count(buffer: &LogBuffer) -> usize {
    buffer
        .events
        .iter()
        .filter(|event| event.action == CAPACITY_WARN_NOTE)
        .count()
}

#[test]
fn push_drops_the_oldest_event_when_full() {
    let mut buffer = LogBuffer::new();
    fill(&mut buffer, CAPACITY + 5);
    assert_eq!(buffer.len(), CAPACITY);
    assert_ne!(buffer.events[0].action, "0");
}

#[test]
fn push_inserts_one_capacity_warning_at_80_percent() {
    let mut buffer = LogBuffer::new();
    fill(&mut buffer, LIMIT - 1);
    assert_eq!(buffer.len(), LIMIT - 1);
    assert_eq!(warn_count(&buffer), 0);
    let before = chrono::Utc::now();
    buffer.push(LogEvent::info("crossing"));
    let after = chrono::Utc::now();
    assert_eq!(buffer.len(), LIMIT + 1);
    assert_eq!(buffer.events[LIMIT - 1].action, "crossing");
    let warn = &buffer.events[LIMIT];
    assert_eq!(warn.action, CAPACITY_WARN_NOTE);
    assert_eq!(warn.level, LogLevel::Warn);
    crate::test::logging::helpers::assert_uuid_v7(&warn.id);
    assert!(warn.context.is_empty());
    assert!(warn.extra.is_empty());
    assert!(warn.ts >= before && warn.ts <= after);
    buffer.push(LogEvent::info("next"));
    assert_eq!(warn_count(&buffer), 1);
}

#[test]
fn push_drops_the_oldest_event_to_store_the_capacity_warning() {
    let mut buffer = LogBuffer::new();
    fill(&mut buffer, CAPACITY);
    assert_eq!(buffer.len(), CAPACITY);
    assert_eq!(warn_count(&buffer), 1);
}
