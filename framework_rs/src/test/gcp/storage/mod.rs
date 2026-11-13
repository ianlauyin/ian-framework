mod list;
mod object;
mod ops_log;
mod put_request;
mod signed_url;

use std::collections::BTreeMap;

use crate::exception;
use crate::gcp::storage::*;

#[test]
fn object_names_are_checked() {
    assert!(validate_object_name("a/b.txt").is_ok());
    assert!(validate_object_name(&"a".repeat(MAX_OBJECT_NAME_BYTES)).is_ok());
    for bad in ["", ".", "..", "a\nb"] {
        assert!(validate_object_name(bad).is_err());
    }
    assert!(validate_object_name(&"a".repeat(MAX_OBJECT_NAME_BYTES + 1)).is_err());
}

#[test]
fn bucket_names_are_checked() {
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

#[test]
fn only_a_missing_object_becomes_none() {
    let missing = Err::<u8, _>(exception!(code = "GCS_NOT_FOUND", "gone"));
    assert_eq!(not_found_as_none(missing).unwrap(), None);
    let other = Err::<u8, _>(exception!(code = "GCP_API", "bad"));
    assert!(not_found_as_none(other).is_err());
    assert_eq!(not_found_as_none(Ok(1_u8)).unwrap(), Some(1));
}
