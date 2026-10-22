use super::super::{send, Call, GcsBackend, GcsError};
use super::shared::{meta_from_response, object_url, validate_object_name, ObjectMeta};

impl GcsBackend {
    pub async fn stat(&self, name: &str) -> Result<ObjectMeta, GcsError> {
        validate_object_name(name)?;
        let response = send(
            self,
            &Call {
                method: reqwest::Method::GET,
                url: object_url(self.bucket(), name),
                headers: Vec::new(),
                json: None,
                bytes: None,
                extra_status: &[],
                timeout: None,
            },
        )
        .await?;
        meta_from_response(response).await
    }
}
