use crate::token::is_token68;

#[test]
fn only_a_token68_text_is_accepted() {
    assert!(is_token68("aZ09-._~+/=="));
    assert!(is_token68("a"));
    for token in ["", "===", "a=b", "a b", "key!value", " a", "a\tb"] {
        assert!(!is_token68(token), "{token:?}");
    }
}
