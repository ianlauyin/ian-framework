use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::Argon2;

use crate::exception;
use crate::exception::Exception;
use crate::exception::Severity;
use crate::http::error_code;

pub const PASSWORD_HASH_FAILED: &str = "PASSWORD_HASH_FAILED";

pub(crate) const MAX_PASSWORD_BYTES: usize = 256;

pub async fn hash_password(password: &str) -> Result<String, Exception> {
    if password.len() > MAX_PASSWORD_BYTES {
        return Err(exception!(
            severity = Severity::Warn,
            code = error_code::VALIDATION_ERROR,
            format!("password must not be longer than {MAX_PASSWORD_BYTES} bytes"),
        ));
    }
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
}

pub async fn verify_password(password: &str, hash: &str) -> bool {
    if password.len() > MAX_PASSWORD_BYTES {
        return false;
    }
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}
