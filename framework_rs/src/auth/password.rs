use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};

use crate::exception;
use crate::exception::Exception;
use crate::exception::Severity;
use crate::http::error_code;

pub const PASSWORD_HASH_FAILED: &str = "PASSWORD_HASH_FAILED";
pub const PASSWORD_HASH_INVALID: &str = "PASSWORD_HASH_INVALID";

pub const MIN_PASSWORD_BYTES: usize = 8;
pub const MAX_PASSWORD_BYTES: usize = 256;

pub async fn hash_password(password: &str) -> Result<String, Exception> {
    validate_password(password)?;
    let password = password.to_string();
    run_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|hash| hash.to_string())
            .map_err(|err| {
                exception!(
                    code = PASSWORD_HASH_FAILED,
                    "password hash failed",
                    source = err
                )
            })
    })
    .await
}

pub async fn verify_password(password: &str, hash: &str) -> Result<bool, Exception> {
    if password.len() > MAX_PASSWORD_BYTES {
        return Err(too_long());
    }
    let password = password.to_string();
    let hash = hash.to_string();
    run_blocking(move || {
        let parsed = PasswordHash::new(&hash).map_err(|err| {
            exception!(
                code = PASSWORD_HASH_INVALID,
                "password hash is not valid",
                source = err
            )
        })?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await
}

pub fn validate_password(password: &str) -> Result<(), Exception> {
    if password.len() < MIN_PASSWORD_BYTES {
        return Err(exception!(
            severity = Severity::Warn,
            code = error_code::VALIDATION_ERROR,
            format!("password must not be shorter than {MIN_PASSWORD_BYTES} bytes"),
        ));
    }
    if password.len() > MAX_PASSWORD_BYTES {
        return Err(too_long());
    }
    let has_letter = password.chars().any(|c| c.is_ascii_alphabetic());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    if !has_letter || !has_digit {
        return Err(exception!(
            severity = Severity::Warn,
            code = error_code::VALIDATION_ERROR,
            "password must mix english letters with digits",
        ));
    }
    Ok(())
}

fn too_long() -> Exception {
    exception!(
        severity = Severity::Warn,
        code = error_code::VALIDATION_ERROR,
        format!("password must not be longer than {MAX_PASSWORD_BYTES} bytes"),
    )
}

async fn run_blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, Exception> + Send + 'static,
) -> Result<T, Exception> {
    tokio::task::spawn_blocking(work).await.map_err(|err| {
        exception!(
            code = PASSWORD_HASH_FAILED,
            "password hash task failed",
            source = err
        )
    })?
}
