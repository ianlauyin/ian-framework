use crate::logging::HttpLogShipper;

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
