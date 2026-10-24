use crate::auth::session_store::{store_err, SessionStore};
use crate::auth::AuthError;

impl SessionStore {
    pub async fn touch(&self, token: &str) -> Result<bool, AuthError> {
        redis::cmd("EXPIRE")
            .arg(self.key(token))
            .arg(self.ttl_secs())
            .query_async(&mut self.conn())
            .await
            .map_err(store_err)
    }
}
