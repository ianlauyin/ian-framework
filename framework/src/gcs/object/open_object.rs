use super::super::{send, Call, GcsBackend, GcsError};
use super::read::ObjectReader;
use super::shared::{object_url, validate_object_name};

impl GcsBackend {
    pub async fn open_object(&self, name: &str) -> Result<ObjectReader, GcsError> {
        validate_object_name(name)?;
        let response = send(
            self,
            &Call {
                method: reqwest::Method::GET,
                url: format!("{}?alt=media", object_url(self.bucket(), name)),
                headers: Vec::new(),
                json: None,
                bytes: None,
                extra_status: &[],
                timeout: None,
            },
        )
        .await?;
        Ok(ObjectReader { response })
    }
}
