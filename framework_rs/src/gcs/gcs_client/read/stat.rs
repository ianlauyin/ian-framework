use crate::gcs::gcs_client::{object_url, validate_object_name, Call, GcsClient};
use crate::gcs::{GcsError, ObjectMeta};

impl GcsClient {
    pub async fn stat(&self, name: &str) -> Result<ObjectMeta, GcsError> {
        validate_object_name(name)?;
        let response = self
            .send(&Call {
                method: reqwest::Method::GET,
                url: object_url(self.bucket(), name),
                ..Default::default()
            })
            .await?;
        ObjectMeta::from_response(response).await
    }
}
