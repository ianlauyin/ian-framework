use crate::exception::error_code;

use crate::auth::password::*;

#[tokio::test]
async fn hash_round_trips_and_uses_a_new_salt() {
    let first = hash_password("correct horse").await.unwrap();
    let second = hash_password("correct horse").await.unwrap();
    assert!(first.starts_with("$argon2id$"));
    assert_ne!(first, second);
    assert!(verify_password("correct horse", &first).await);
    assert!(!verify_password("wrong horse", &first).await);
    assert!(!verify_password("correct horse", "not a phc string").await);
}

#[tokio::test]
async fn long_passwords_are_refused() {
    let limit = "a".repeat(MAX_PASSWORD_BYTES);
    let hash = hash_password(&limit).await.unwrap();
    assert!(verify_password(&limit, &hash).await);
    let long = "a".repeat(MAX_PASSWORD_BYTES + 1);
    let error = hash_password(&long).await.unwrap_err();
    assert_eq!(error.code(), error_code::VALIDATION_ERROR);
    assert_eq!(
        error.message(),
        "password must not be longer than 256 bytes"
    );
    assert!(!verify_password(&long, &hash).await);
}
