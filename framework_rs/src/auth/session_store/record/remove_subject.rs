use redis::AsyncCommands;

use crate::auth::session_store::{parse_value, store_err, SessionStore};
use crate::auth::AuthError;

impl SessionStore {
    pub async fn remove_subject(&self, subject: &str) -> Result<(), AuthError> {
        self.remove_subject_keys(subject, None).await
    }

    pub(super) async fn remove_subject_keys(
        &self,
        subject: &str,
        keep_key: Option<&str>,
    ) -> Result<(), AuthError> {
        let mut conn = self.conn();
        let pattern = format!("{}*", escape_glob(&self.inner.config.prefix));
        let mut keys = Vec::new();
        {
            let mut iter = conn
                .scan_match::<_, String>(pattern)
                .await
                .map_err(store_err)?;
            while let Some(key) = iter.next_item().await {
                keys.push(key.map_err(store_err)?);
            }
        }
        for key in keys {
            if keep_key == Some(key.as_str()) {
                continue;
            }
            let value: Option<String> = redis::cmd("GET")
                .arg(&key)
                .query_async(&mut conn)
                .await
                .map_err(store_err)?;
            let owner = value
                .as_deref()
                .and_then(parse_value)
                .map(|(_, owner)| owner);
            if owner == Some(subject) {
                redis::cmd("DEL")
                    .arg(&key)
                    .query_async::<()>(&mut conn)
                    .await
                    .map_err(store_err)?;
            }
        }
        Ok(())
    }
}

fn escape_glob(prefix: &str) -> String {
    let mut out = String::with_capacity(prefix.len());
    for ch in prefix.chars() {
        if matches!(ch, '*' | '?' | '[' | ']' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_characters_in_the_prefix_are_escaped() {
        assert_eq!(escape_glob("ics:session:"), "ics:session:");
        assert_eq!(escape_glob("a*b?[c]\\"), "a\\*b\\?\\[c\\]\\\\");
    }
}
