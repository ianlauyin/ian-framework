use crate::exception;
use crate::gcp::storage::client::*;

#[test]
fn only_a_missing_object_becomes_none() {
    let missing = Err::<u8, _>(exception!(code = "RESOURCE_NOT_FOUND", "gone"));
    assert_eq!(not_found_as_none(missing).unwrap(), None);
    let other = Err::<u8, _>(exception!(code = "API_FAILED", "bad"));
    assert!(not_found_as_none(other).is_err());
    assert_eq!(not_found_as_none(Ok(1_u8)).unwrap(), Some(1));
}
