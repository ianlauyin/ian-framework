use std::collections::BTreeMap;

use crate::gcs::gcs_client::{
    object_url, validate_metadata, validate_object_name, Call, GcsClient,
};
use crate::gcs::{GcsError, ObjectMeta};

impl GcsClient {
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
                json: Some(serde_json::json!({ "metadata": metadata })),
                ..Default::default()
            })
            .await?;
        ObjectMeta::from_response(response).await
    }
}
