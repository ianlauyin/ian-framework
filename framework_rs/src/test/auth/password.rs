use crate::http::error_code;

use crate::auth::password::*;

#[tokio::test]
async fn hash_round_trips_and_uses_a_new_salt() {
    let first = hash_password("correct horse 1").await.unwrap();
    let second = hash_password("correct horse 1").await.unwrap();
    assert!(first.starts_with("$argon2id$"));
    assert_ne!(first, second);
    assert!(verify_password("correct horse 1", &first).await);
    assert!(!verify_password("wrong horse 1", &first).await);
    assert!(!verify_password("correct horse 1", "not a phc string").await);
}

#[tokio::test]
async fn invalid_passwords_are_refused_by_hash_and_verify() {
    let hash = hash_password("correct horse 1").await.unwrap();
    assert!(hash_password("short1").await.is_err());
    assert!(hash_password("onlyletters").await.is_err());
    assert!(!verify_password("short1", &hash).await);
    assert!(!verify_password("onlyletters", &hash).await);
}

#[test]
fn validate_accepts_the_length_limits() {
    assert!(validate_password("abcdefg1").is_ok());
    assert!(validate_password(&format!("{}1", "a".repeat(MAX_PASSWORD_BYTES - 1))).is_ok());
}

#[test]
fn validate_refuses_short_passwords() {
    let error = validate_password("abcdef1").unwrap_err();
    assert_eq!(error.code(), error_code::VALIDATION_ERROR);
    assert_eq!(error.message(), "password must not be shorter than 8 bytes");
}

#[test]
fn validate_refuses_long_passwords() {
    let long = format!("{}1", "a".repeat(MAX_PASSWORD_BYTES));
    let error = validate_password(&long).unwrap_err();
    assert_eq!(error.code(), error_code::VALIDATION_ERROR);
    assert_eq!(
        error.message(),
        "password must not be longer than 256 bytes"
    );
}

#[test]
fn validate_refuses_passwords_without_a_letter_or_a_digit() {
    for password in ["12345678", "abcdefgh", "密码密码密码1"] {
        let error = validate_password(password).unwrap_err();
        assert_eq!(error.code(), error_code::VALIDATION_ERROR);
        assert_eq!(
            error.message(),
            "password must mix english letters with digits"
        );
    }
}
