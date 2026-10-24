use crate::gcs::gcs_backend::{object_url, validate_object_name, Call, GcsBackend};
use crate::gcs::{GcsError, ObjectMeta};

impl GcsBackend {
    pub async fn stat(&self, name: &str) -> Result<ObjectMeta, GcsError> {
        validate_object_name(name)?;
        let response = self
            .send(&Call {
                method: reqwest::Method::GET,
                url: object_url(self.bucket(), name),
                headers: Vec::new(),
                json: None,
                bytes: None,
                extra_status: &[],
                timeout: None,
            })
            .await?;
        ObjectMeta::from_response(response).await
    }
}
