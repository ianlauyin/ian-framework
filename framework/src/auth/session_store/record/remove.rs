use crate::auth::session_store::{store_err, SessionStore};
use crate::auth::AuthError;

impl SessionStore {
    pub async fn remove(&self, token: &str) -> Result<(), AuthError> {
        redis::cmd("DEL")
            .arg(self.key(token))
            .query_async::<()>(&mut self.conn())
            .await
            .map_err(store_err)
    }
}
