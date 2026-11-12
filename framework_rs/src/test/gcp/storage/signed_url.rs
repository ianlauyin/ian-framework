use std::time::Duration;

use crate::gcp::storage::signed_url::*;

#[test]
fn a_lifetime_is_between_one_second_and_seven_days() {
    assert!(validate_lifetime(Duration::ZERO).is_err());
    assert!(validate_lifetime(Duration::from_millis(999)).is_err());
    assert_eq!(
        validate_lifetime(Duration::from_secs(1)).unwrap(),
        Duration::from_secs(1)
    );
    assert!(validate_lifetime(Duration::from_secs(604_800)).is_ok());
    assert!(validate_lifetime(Duration::from_secs(604_801)).is_err());
}

#[test]
fn a_disposition_has_an_ascii_fallback_and_an_encoded_name() {
    assert_eq!(
        content_disposition_value("a\"b\u{e9}.txt").unwrap(),
        "attachment; filename=\"a_b_.txt\"; filename*=UTF-8''a%22b%C3%A9.txt"
    );
}

#[test]
fn an_empty_filename_is_refused() {
    assert!(content_disposition_value("").is_err());
}
