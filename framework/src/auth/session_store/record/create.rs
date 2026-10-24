use crate::auth::session_store::{store_err, SessionStore};
use crate::auth::{AuthError, AuthErrorKind};

impl SessionStore {
    pub async fn create(&self, subject: &str) -> Result<String, AuthError> {
        let token = random_token()?;
        redis::cmd("SET")
            .arg(self.key(&token))
            .arg(subject)
            .arg("EX")
            .arg(self.ttl_secs())
            .query_async::<()>(&mut self.conn())
            .await
            .map_err(store_err)?;
        Ok(token)
    }
}

fn random_token() -> Result<String, AuthError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|err| {
        AuthError::new(
            AuthErrorKind::Random,
            format!("os random source failed: {err}"),
        )
    })?;
    Ok(hex::encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_64_lowercase_hex_characters() {
        let token = random_token().unwrap();
        assert_eq!(token.len(), 64);
        assert!(token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
        assert_ne!(token, random_token().unwrap());
    }
}
