use super::super::encode::{encode_object_id, encode_seg};
use super::super::{send, Call, GcsBackend, GcsError};
use super::shared::{meta_from_response, object_url, validate_object_name, ObjectMeta};

impl GcsBackend {
    pub async fn copy_object(&self, from: &str, to: &str) -> Result<ObjectMeta, GcsError> {
        validate_object_name(from)?;
        validate_object_name(to)?;
        let url = format!(
            "{}/copyTo/b/{}/o/{}",
            object_url(self.bucket(), from),
            encode_seg(self.bucket()),
            encode_object_id(to),
        );
        let response = send(
            self,
            &Call {
                method: reqwest::Method::POST,
                url,
                headers: Vec::new(),
                json: Some(serde_json::json!({})),
                bytes: None,
                extra_status: &[],
                timeout: None,
            },
        )
        .await?;
        meta_from_response(response).await
    }
}
