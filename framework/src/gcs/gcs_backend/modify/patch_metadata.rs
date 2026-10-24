use std::collections::BTreeMap;

use crate::gcs::gcs_backend::{
    object_url, validate_metadata, validate_object_name, Call, GcsBackend,
};
use crate::gcs::{GcsError, ObjectMeta};

impl GcsBackend {
    pub async fn patch_metadata(
        &self,
        name: &str,
        metadata: &BTreeMap<String, String>,
    ) -> Result<ObjectMeta, GcsError> {
        validate_object_name(name)?;
        validate_metadata(metadata)?;
        let response = self
            .send(&Call {
                method: reqwest::Method::PATCH,
                url: object_url(self.bucket(), name),
                headers: Vec::new(),
                json: Some(serde_json::json!({ "metadata": metadata })),
                bytes: None,
                extra_status: &[],
                timeout: None,
            })
            .await?;
        ObjectMeta::from_response(response).await
    }
}
