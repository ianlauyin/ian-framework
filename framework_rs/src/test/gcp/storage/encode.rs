use crate::gcp::storage::encode::*;

#[test]
fn only_the_unreserved_characters_stay() {
    assert_eq!(percent_encode("aZ09-._~"), "aZ09-._~");
    assert_eq!(percent_encode("a b/\u{e9}"), "a%20b%2F%C3%A9");
}
