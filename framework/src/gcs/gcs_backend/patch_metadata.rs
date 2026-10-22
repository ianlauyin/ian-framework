use std::collections::BTreeMap;

use super::super::{send, Call, GcsBackend, GcsError};
use super::shared::{
    meta_from_response, object_url, validate_metadata, validate_object_name, ObjectMeta,
};

impl GcsBackend {
    pub async fn patch_metadata(
        &self,
        name: &str,
        metadata: &BTreeMap<String, String>,
    ) -> Result<ObjectMeta, GcsError> {
        validate_object_name(name)?;
        validate_metadata(metadata)?;
        let response = send(
            self,
            &Call {
                method: reqwest::Method::PATCH,
                url: object_url(self.bucket(), name),
                headers: Vec::new(),
                json: Some(serde_json::json!({ "metadata": metadata })),
                bytes: None,
                extra_status: &[],
                timeout: None,
            },
        )
        .await?;
        meta_from_response(response).await
    }
}
