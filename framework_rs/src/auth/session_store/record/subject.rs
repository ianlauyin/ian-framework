use crate::auth::session_store::{now_secs, parse_value, store_err, SessionStore};
use crate::auth::AuthError;

impl SessionStore {
    pub async fn subject(&self, token: &str) -> Result<Option<String>, AuthError> {
        let value: Option<String> = redis::cmd("GET")
            .arg(self.key(token))
            .query_async(&mut self.conn())
            .await
            .map_err(store_err)?;
        let subject = value
            .as_deref()
            .and_then(parse_value)
            .and_then(|(deadline, subject)| (deadline > now_secs()).then(|| subject.to_string()));
        Ok(subject)
    }
}
