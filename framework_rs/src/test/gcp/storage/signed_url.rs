use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::gcp::storage::signed_url::*;

#[test]
fn only_the_unreserved_characters_stay() {
    assert_eq!(percent_encode("aZ09-._~"), "aZ09-._~");
    assert_eq!(percent_encode("a b/\u{e9}"), "a%20b%2F%C3%A9");
}

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

#[test]
fn an_attachment_query_encodes_the_asterisk() {
    let now = DateTime::parse_from_rfc3339("2026-11-15T15:35:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let disposition = content_disposition_value("a.jpg").unwrap();
    let signed = signed_get(
        "bucket",
        "dir/a b.jpg",
        "signer@example.com",
        3600,
        now,
        Some(&disposition),
    );
    let query = "X-Goog-Algorithm=GOOG4-RSA-SHA256&X-Goog-Credential=signer%40example.com%2F20261115%2Fauto%2Fstorage%2Fgoog4_request&X-Goog-Date=20261115T153500Z&X-Goog-Expires=3600&X-Goog-SignedHeaders=host&response-content-disposition=attachment%3B%20filename%3D%22a.jpg%22%3B%20filename%2A%3DUTF-8%27%27a.jpg";
    assert_eq!(
        signed.url,
        format!("https://storage.googleapis.com/bucket/dir/a%20b.jpg?{query}")
    );
    assert_eq!(
        signed.string_to_sign,
        "GOOG4-RSA-SHA256\n20261115T153500Z\n20261115/auto/storage/goog4_request\n5c4a83737843acbbd16a935468365dc13dbe8b3dda138b6d3b5fdc3cbb97a2ae"
    );
    assert!(!signed.url.contains("filename*"));
}

#[test]
fn a_percent_in_the_disposition_is_encoded_again() {
    let disposition = content_disposition_value("a\"b\u{e9}.txt").unwrap();
    let signed = signed_get(
        "bucket",
        "o",
        "signer@example.com",
        1,
        fixed_stamp(),
        Some(&disposition),
    );
    assert!(
        signed
            .url
            .contains("filename%2A%3DUTF-8%27%27a%2522b%25C3%25A9.txt")
    );
}

#[test]
fn a_preview_url_has_no_disposition() {
    let signed = signed_get("bucket", "o", "signer@example.com", 1, fixed_stamp(), None);
    assert!(!signed.url.contains("response-content-disposition"));
    assert!(signed.url.contains("X-Goog-SignedHeaders=host"));
}

#[test]
fn a_path_keeps_a_slash_and_encodes_other_bytes() {
    assert_eq!(encode_path("dir/\u{4e16}/a~b.jpg"), "dir/%E4%B8%96/a~b.jpg");
}

fn fixed_stamp() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-11-15T15:35:00Z")
        .unwrap()
        .with_timezone(&Utc)
}
