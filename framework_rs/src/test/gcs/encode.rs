use crate::gcs::encode::*;

#[test]
fn id_encodes_slashes() {
    assert_eq!(
        encode_object_id("photos/my cat.jpeg"),
        "photos%2Fmy%20cat.jpeg"
    );
    assert_eq!(encode_seg("é"), "%C3%A9");
    assert_eq!(encode_seg("+"), "%2B");
}
