use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::Argon2;

use super::{AuthError, AuthErrorKind};

const MAX_PASSWORD_BYTES: usize = 256;

pub fn hash_password(password: &str) -> Result<String, AuthError> {
    if password.len() > MAX_PASSWORD_BYTES {
        return Err(AuthError::new(
            AuthErrorKind::Password,
            "password is too long",
        ));
    }
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|err| AuthError::with_source(AuthErrorKind::Password, "password hash failed", err))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_round_trips_and_uses_a_new_salt() {
        let first = hash_password("correct horse").unwrap();
        let second = hash_password("correct horse").unwrap();
        assert!(first.starts_with("$argon2id$"));
        assert_ne!(first, second);
        assert!(verify_password("correct horse", &first));
        assert!(!verify_password("wrong horse", &first));
        assert!(!verify_password("correct horse", "not a phc string"));
    }

    #[test]
    fn long_passwords_are_refused() {
        let limit = "a".repeat(MAX_PASSWORD_BYTES);
        let hash = hash_password(&limit).unwrap();
        assert!(verify_password(&limit, &hash));
        let long = "a".repeat(MAX_PASSWORD_BYTES + 1);
        assert_eq!(
            hash_password(&long).unwrap_err().kind(),
            AuthErrorKind::Password
        );
        assert!(!verify_password(&long, &hash));
    }
}
