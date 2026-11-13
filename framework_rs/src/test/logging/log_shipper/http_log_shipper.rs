use std::time::Duration;

use crate::logging::log_shipper::http_log_shipper::SHIP_TIMEOUT;
use crate::logging::log_shipper::SHIP_CHUNK;
use crate::logging::logger::log_buffer::CAPACITY;
use crate::logging::logger::SHIP_EVERY;
use crate::logging::HttpLogShipper;
use crate::test::logging::default_shipper;

#[test]
fn defaults() {
    let shipper = default_shipper("api");
    assert_eq!(shipper.inner.app, "api");
    assert_eq!(SHIP_CHUNK, 1_000);
    assert_eq!(SHIP_TIMEOUT, Duration::from_secs(10));
    assert_eq!(CAPACITY, 20_000);
    assert_eq!(SHIP_EVERY, Duration::from_secs(60));
}

#[test]
fn invalid_post_api_key_returns_an_exception() {
    let err = HttpLogShipper::new("api", "https://logs.example".to_string(), "log\u{7f}key")
        .err()
        .unwrap();
    assert_eq!(err.code(), "API_KEY_HEADER_INVALID");
    assert!(err.message().contains("not a valid header value"));
    assert!(!format!("{err:?}").contains("log\u{7f}key"));
}

#[test]
fn debug_hides_the_post_api_key() {
    let debug = format!(
        "{:?}",
        HttpLogShipper::new("api", "https://logs.example".to_string(), "log-post-key").unwrap()
    );
    assert!(!debug.contains("log-post-key"));
    assert!(debug.contains("post_api_key: \"***\""));
}
