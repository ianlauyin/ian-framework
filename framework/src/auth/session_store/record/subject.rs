use crate::auth::session_store::{store_err, SessionStore};
use crate::auth::AuthError;

impl SessionStore {
    pub async fn subject(&self, token: &str) -> Result<Option<String>, AuthError> {
        redis::cmd("GET")
            .arg(self.key(token))
            .query_async(&mut self.conn())
            .await
            .map_err(store_err)
    }
}
