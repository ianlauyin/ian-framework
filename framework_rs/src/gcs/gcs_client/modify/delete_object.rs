use crate::gcs::gcs_client::{object_url, validate_object_name, Call, GcsClient};
use crate::gcs::GcsError;

impl GcsClient {
    pub async fn delete_object(&self, name: &str) -> Result<(), GcsError> {
        validate_object_name(name)?;
        let response = self
            .send(&Call {
                method: reqwest::Method::DELETE,
                url: object_url(self.bucket(), name),
                ..Default::default()
            })
            .await?;
        let _ = response.bytes().await;
        Ok(())
    }
}
