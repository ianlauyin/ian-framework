use super::super::{send, Call, GcsBackend, GcsError};
use super::shared::{object_url, validate_object_name};

impl GcsBackend {
    pub async fn delete_object(&self, name: &str) -> Result<(), GcsError> {
        validate_object_name(name)?;
        let response = send(
            self,
            &Call {
                method: reqwest::Method::DELETE,
                url: object_url(self.bucket(), name),
                headers: Vec::new(),
                json: None,
                bytes: None,
                extra_status: &[],
                timeout: None,
            },
        )
        .await?;
        let _ = response.bytes().await;
        Ok(())
    }
}
