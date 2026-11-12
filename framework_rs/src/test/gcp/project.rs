use crate::gcp::project::*;

#[test]
fn a_name_segment_has_only_safe_characters() {
    for good in ["my-project", "KEY_1", "a.b"] {
        assert!(is_safe_name(good));
    }
    for bad in ["", "a/b", "a b", "a:b", "k\u{e9}y"] {
        assert!(!is_safe_name(bad));
    }
}
