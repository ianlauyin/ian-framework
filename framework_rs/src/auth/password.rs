use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::Argon2;

use crate::exception;
use crate::exception::Exception;
use crate::exception::Severity;
use crate::http::error_code;

pub(crate) const MAX_PASSWORD_BYTES: usize = 256;

// Argon2 takes tens of milliseconds of CPU, so it runs on the blocking pool and does not stall the async workers.
pub async fn hash_password(password: &str) -> Result<String, Exception> {
    if password.len() > MAX_PASSWORD_BYTES {
        return Err(exception!(
            format!("password must not be longer than {MAX_PASSWORD_BYTES} bytes"),
            code = error_code::VALIDATION_ERROR,
            severity = Severity::Warn,
        ));
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
