use crate::gcs::gcs_client::{object_url, validate_object_name, Call, GcsClient};
use crate::gcs::{GcsError, ObjectReader};

impl GcsClient {
    pub async fn open_object(&self, name: &str) -> Result<ObjectReader, GcsError> {
        validate_object_name(name)?;
        let response = self
            .send(&Call {
                method: reqwest::Method::GET,
                url: format!("{}?alt=media", object_url(self.bucket(), name)),
                headers: Vec::new(),
                json: None,
                bytes: None,
                extra_status: &[],
                timeout: None,
            })
            .await?;
        Ok(ObjectReader { response })
    }
}
