use crate::auth::session_store::SessionStore;
use crate::auth::AuthError;

impl SessionStore {
    pub async fn remove_subject_except(
        &self,
        subject: &str,
        keep_token: &str,
    ) -> Result<(), AuthError> {
        self.remove_subject_keys(subject, Some(&self.key(keep_token)))
            .await
    }
}
