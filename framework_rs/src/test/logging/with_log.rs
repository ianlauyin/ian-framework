use crate::logging::*;
use crate::test::logging::{buffered_events, global_logger};

#[tokio::test]
async fn it_returns_the_output_and_records_the_event_with_the_latency() {
    let logger = global_logger();
    let output = with_log(
        async {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            7
        },
        |output| {
            let mut event = LogEvent::info("with-log/records");
            event.latency_ms = 999_999;
            event
                .extra
                .insert("output".to_string(), (*output as u64).into());
            event
        },
    )
    .await;
    assert_eq!(output, 7);
    let events: Vec<_> = buffered_events(logger)
        .into_iter()
        .filter(|event| event.action == "with-log/records")
        .collect();
    assert_eq!(events.len(), 1);
    assert!((20..999_999).contains(&events[0].latency_ms));
    assert_eq!(events[0].extra.get("output"), Some(&LogValue::from(7_u64)));
}
