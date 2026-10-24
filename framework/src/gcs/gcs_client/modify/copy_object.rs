use crate::gcs::encode::{encode_object_id, encode_seg};
use crate::gcs::gcs_client::{object_url, validate_object_name, Call, GcsClient};
use crate::gcs::{GcsError, ObjectMeta};

impl GcsClient {
    pub async fn copy_object(&self, from: &str, to: &str) -> Result<ObjectMeta, GcsError> {
        validate_object_name(from)?;
        validate_object_name(to)?;
        let url = format!(
            "{}/copyTo/b/{}/o/{}",
            object_url(self.bucket(), from),
            encode_seg(self.bucket()),
            encode_object_id(to),
        );
        let response = self
            .send(&Call {
                method: reqwest::Method::POST,
                url,
                headers: Vec::new(),
                json: Some(serde_json::json!({})),
                bytes: None,
                extra_status: &[],
                timeout: None,
            })
            .await?;
        ObjectMeta::from_response(response).await
    }
}
