use crate::auth::session::token::*;

#[test]
fn token_is_64_lowercase_hex_characters() {
    let token = random_token().unwrap();
    assert_eq!(token.len(), 64);
    assert!(token
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    assert_ne!(token, random_token().unwrap());
}

#[test]
fn accepts_only_the_token_shape() {
    assert!(is_token(&"a1".repeat(32)));
    assert!(!is_token(&"a1".repeat(31)));
    assert!(!is_token(&"A1".repeat(32)));
    assert!(!is_token(&"g1".repeat(32)));
    assert!(!is_token(""));
}
