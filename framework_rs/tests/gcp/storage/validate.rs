use std::collections::BTreeMap;

use ian_framework_rs::gcp::storage::validate::*;

#[test]
fn an_empty_dot_long_or_multiline_object_name_is_refused() {
    assert!(validate_object_name("a/b.txt").is_ok());
    assert!(validate_object_name(&"a".repeat(MAX_OBJECT_NAME_BYTES)).is_ok());
    for bad in ["", ".", "..", "a\nb"] {
        assert!(validate_object_name(bad).is_err());
    }
    assert!(validate_object_name(&"a".repeat(MAX_OBJECT_NAME_BYTES + 1)).is_err());
}

#[test]
fn a_bucket_name_with_a_separator_space_or_non_ascii_is_refused() {
    assert!(validate_bucket("my-bucket").is_ok());
    for bad in ["", "a/b", "a b", "a\\b", "b\u{fc}cket"] {
        assert!(validate_bucket(bad).is_err());
    }
}

#[test]
fn metadata_must_have_no_empty_key_and_no_line_break() {
    let map = |key: &str, value: &str| BTreeMap::from([(key.to_string(), value.to_string())]);
    assert!(validate_metadata(&map("k", "v")).is_ok());
    assert!(validate_metadata(&map("", "v")).is_err());
    assert!(validate_metadata(&map("k\n", "v")).is_err());
    assert!(validate_metadata(&map("k", "v\r")).is_err());
}
