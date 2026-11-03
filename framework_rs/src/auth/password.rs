use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::Argon2;
use tokio::sync::Semaphore;

use super::{AuthError, AuthErrorKind};

const MAX_PASSWORD_BYTES: usize = 256;
const MAX_CONCURRENT_HASHES: usize = 8;

// Each Argon2 run uses much memory and CPU, so a flood of logins must wait here and not fill the blocking pool.
static HASH_PERMITS: Semaphore = Semaphore::const_new(MAX_CONCURRENT_HASHES);

// Argon2 takes tens of milliseconds of CPU, so it runs on the blocking pool and does not stall the async workers.
pub async fn hash_password(password: &str) -> Result<String, AuthError> {
    if password.len() > MAX_PASSWORD_BYTES {
        return Err(AuthError::new(
            AuthErrorKind::Password,
            "password is too long",
        ));
    }
    let password = password.to_owned();
    let _permit = HASH_PERMITS.acquire().await.map_err(|err| {
        AuthError::with_source(
            AuthErrorKind::Password,
            "password hash is not available",
            err,
        )
    })?;
    tokio::task::spawn_blocking(move || hash(&password))
        .await
        .map_err(|err| {
            AuthError::with_source(AuthErrorKind::Password, "password hash task failed", err)
        })?
}

pub async fn verify_password(password: &str, hash: &str) -> bool {
    if password.len() > MAX_PASSWORD_BYTES {
        return false;
    }
    let password = password.to_owned();
    let hash = hash.to_owned();
    let Ok(_permit) = HASH_PERMITS.acquire().await else {
        return false;
    };
    tokio::task::spawn_blocking(move || verify(&password, &hash))
        .await
        .unwrap_or(false)
}

fn hash(password: &str) -> Result<String, AuthError> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|err| AuthError::with_source(AuthErrorKind::Password, "password hash failed", err))
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
        assert_eq!(
            hash_password(&long).await.unwrap_err().kind(),
            AuthErrorKind::Password
        );
        assert!(!verify_password(&long, &hash).await);
    }
}
