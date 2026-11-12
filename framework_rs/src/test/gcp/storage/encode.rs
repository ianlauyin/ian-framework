use crate::gcp::storage::encode::*;

#[test]
fn segment_encodes_slashes_and_non_ascii() {
    assert_eq!(encode_seg("photos/my cat.jpeg"), "photos%2Fmy%20cat.jpeg");
    assert_eq!(encode_seg("é"), "%C3%A9");
    assert_eq!(encode_seg("+"), "%2B");
}
