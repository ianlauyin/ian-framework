use base64::Engine;

use crate::gcs::encode::encode_seg;
use crate::gcs::gcs_client::{Call, GcsClient};
use crate::gcs::{GcsError, GcsErrorKind};

impl GcsClient {
    pub(crate) async fn sign_blob(&self, payload: &[u8]) -> Result<Vec<u8>, GcsError> {
        let url = format!(
            "https://iamcredentials.googleapis.com/v1/projects/-/serviceAccounts/{}:signBlob",
            encode_seg(self.signer_email()),
        );
        let encoded = base64::engine::general_purpose::STANDARD.encode(payload);
        let response = self
            .send(&Call {
                method: reqwest::Method::POST,
                url,
                headers: Vec::new(),
                json: Some(serde_json::json!({ "payload": encoded })),
                bytes: None,
                extra_status: &[],
                timeout: None,
            })
            .await?;
        let bytes = response.bytes().await.map_err(|err| {
            GcsError::with_source(
                GcsErrorKind::Transport,
                "signBlob response is not bytes",
                err,
            )
        })?;
        let parsed: SignBlobResponse = serde_json::from_slice(&bytes).map_err(|err| {
            GcsError::with_source(GcsErrorKind::Api, "signBlob response is not JSON", err)
        })?;
        let cleaned: String = parsed
            .signed_blob
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect();
        let signature = base64::engine::general_purpose::STANDARD
            .decode(cleaned)
            .map_err(|err| {
                GcsError::with_source(GcsErrorKind::Api, "signBlob signature is not base64", err)
            })?;
        if signature.is_empty() {
            return Err(GcsError::new(
                GcsErrorKind::Api,
                "signBlob returned an empty signature",
            ));
        }
        Ok(signature)
    }
}

#[derive(serde::Deserialize)]
struct SignBlobResponse {
    #[serde(rename = "signedBlob")]
    signed_blob: String,
}
