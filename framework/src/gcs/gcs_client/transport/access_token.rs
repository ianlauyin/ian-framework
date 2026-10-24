use std::time::Instant;

use crate::logging::LogLevel;

use crate::gcs::gcs_client::{logged_error, GcsClient, OpsRecord};
use crate::gcs::GcsError;

impl GcsClient {
    pub(crate) async fn access_token(&self) -> Result<String, GcsError> {
        let mut guard = self.inner.token.lock().await;
        if let Some(cached) = guard.as_ref() {
            if Instant::now() < cached.expires_at && !cached.value.is_empty() {
                return Ok(cached.value.clone());
            }
        }
        let fetched = self.inner.auth.fetch_token(&self.inner.http).await;
        if let Err(err) = &fetched {
            let error_text = logged_error(err);
            self.record_op(OpsRecord {
                level: LogLevel::Warn,
                message: "access token request failed",
                method: "",
                url: "",
                status: err.status(),
                latency_ms: None,
                extra: &[("error", &error_text)],
            });
        }
        let token = fetched?;
        let value = token.value.clone();
        *guard = Some(token);
        Ok(value)
    }

    pub(crate) async fn invalidate_token(&self) {
        *self.inner.token.lock().await = None;
    }
}
