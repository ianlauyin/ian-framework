use crate::auth::session_store::{now_secs, parse_value, store_err, SessionStore};
use crate::auth::AuthError;

impl SessionStore {
    pub async fn touch(&self, token: &str) -> Result<bool, AuthError> {
        let key = self.key(token);
        let mut conn = self.conn();
        let value: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(store_err)?;
        let Some((deadline, _)) = value.as_deref().and_then(parse_value) else {
            return Ok(false);
        };
        let left = deadline.saturating_sub(now_secs());
        if left == 0 {
            return Ok(false);
        }
        redis::cmd("EXPIRE")
            .arg(&key)
            .arg(self.ttl_secs().min(left))
            .query_async(&mut conn)
            .await
            .map_err(store_err)
    }
}
