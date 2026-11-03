use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::Argon2;

use crate::exception;
use crate::exception::Exception;
use crate::validation_error;

const MAX_PASSWORD_BYTES: usize = 256;

// Argon2 takes tens of milliseconds of CPU, so it runs on the blocking pool and does not stall the async workers.
pub async fn hash_password(password: &str) -> Result<String, Exception> {
    if password.len() > MAX_PASSWORD_BYTES {
        return Err(validation_error!(format!(
            "password must not be longer than {MAX_PASSWORD_BYTES} bytes"
        )));
    }
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || hash(&password))
        .await
        .map_err(|err| {
            exception!(
                "password hash task failed",
                code = "AUTH_PASSWORD",
                source = err
            )
        })?
}

pub async fn verify_password(password: &str, hash: &str) -> bool {
    if password.len() > MAX_PASSWORD_BYTES {
        return false;
    }
    let password = password.to_owned();
    let hash = hash.to_owned();
    tokio::task::spawn_blocking(move || verify(&password, &hash))
        .await
        .unwrap_or(false)
}

fn hash(password: &str) -> Result<String, Exception> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|err| exception!("password hash failed", code = "AUTH_PASSWORD", source = err))
}

fn verify(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exception::error_code;

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
        assert_eq!(error.code(), Some(error_code::VALIDATION_ERROR));
        assert_eq!(
            error.message(),
            "password must not be longer than 256 bytes"
        );
        assert!(!verify_password(&long, &hash).await);
    }
}
