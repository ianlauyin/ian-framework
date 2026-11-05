use std::time::Duration;

use base64::Engine;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

use crate::exception;
use crate::exception::Exception;
use crate::gcs::encode::encode_seg;
use crate::gcs::gcs_client::request::Call;
use crate::gcs::gcs_client::GcsClient;

const HOST: &str = "storage.googleapis.com";
pub(crate) const MAX_SIGNED_URL_SECONDS: u64 = 604_800;

pub(crate) struct SignedParts {
    pub(crate) string_to_sign: String,
    pub(crate) url_prefix: String,
}

impl SignedParts {
    pub(crate) fn v4_get(
        bucket_name: &str,
        name: &str,
        email: &str,
        now: DateTime<Utc>,
        expires_in: Option<Duration>,
        attachment_filename: Option<&str>,
    ) -> Result<Self, Exception> {
        let expires_secs = signed_lifetime_seconds(expires_in)?;
        let disposition = attachment_filename
            .map(attachment_disposition)
            .transpose()?;
        let request_ts = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date = now.format("%Y%m%d").to_string();
        let scope = format!("{date}/auto/storage/goog4_request");
        let credential = format!("{email}/{scope}");
        let canonical_uri = format!("/{}/{}", encode_seg(bucket_name), encode_object_path(name));
        let mut pairs = vec![
            ("X-Goog-Algorithm", "GOOG4-RSA-SHA256".to_string()),
            ("X-Goog-Credential", credential),
            ("X-Goog-Date", request_ts.clone()),
            ("X-Goog-Expires", expires_secs.to_string()),
            ("X-Goog-SignedHeaders", "host".to_string()),
        ];
        if let Some(disposition) = disposition {
            pairs.push(("response-content-disposition", disposition));
        }
        pairs.sort_by(|left, right| left.0.cmp(right.0));
        let mut query = String::new();
        for (index, (key, value)) in pairs.iter().enumerate() {
            if index > 0 {
                query.push('&');
            }
            query.push_str(&encode_seg(key));
            query.push('=');
            query.push_str(&encode_seg(value));
        }
        let canonical_request =
            format!("GET\n{canonical_uri}\n{query}\nhost:{HOST}\n\nhost\nUNSIGNED-PAYLOAD");
        let hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));
        let string_to_sign = format!("GOOG4-RSA-SHA256\n{request_ts}\n{scope}\n{hash}");
        let url_prefix = format!("https://{HOST}{canonical_uri}?{query}");
        Ok(Self {
            string_to_sign,
            url_prefix,
        })
    }

    pub(crate) async fn sign(self, client: &GcsClient) -> Result<String, Exception> {
        let signature = sign_blob(client, self.string_to_sign.as_bytes()).await?;
        Ok(format!(
            "{}&X-Goog-Signature={}",
            self.url_prefix,
            hex::encode(signature)
        ))
    }
}

pub(crate) async fn sign_blob(client: &GcsClient, payload: &[u8]) -> Result<Vec<u8>, Exception> {
    let url = format!(
        "{}/projects/-/serviceAccounts/{}:signBlob",
        client.inner.endpoints.iam_root,
        encode_seg(client.signer_email()),
    );
    let encoded = base64::engine::general_purpose::STANDARD.encode(payload);
    let response = client
        .send(&Call {
            method: reqwest::Method::POST,
            url,
            json: Some(serde_json::json!({ "payload": encoded })),
            ..Default::default()
        })
        .await?;
    let bytes = response.bytes().await.map_err(|err| {
        exception!(
            code = "GCS_TRANSPORT",
            "signBlob response is not bytes",
            source = err.without_url(),
        )
    })?;
    signature_from_response(&bytes)
}

pub(crate) fn signature_from_response(bytes: &[u8]) -> Result<Vec<u8>, Exception> {
    let parsed: SignBlobResponse = serde_json::from_slice(bytes).map_err(|err| {
        exception!(
            code = "GCS_API",
            "signBlob response is not JSON",
            source = err,
        )
    })?;
    let cleaned: String = parsed
        .signed_blob
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    let signature = base64::engine::general_purpose::STANDARD
        .decode(cleaned)
        .map_err(|err| {
            exception!(
                code = "GCS_API",
                "signBlob signature is not base64",
                source = err,
            )
        })?;
    if signature.is_empty() {
        return Err(exception!(
            code = "GCS_API",
            "signBlob returned an empty signature",
        ));
    }
    Ok(signature)
}

#[derive(serde::Deserialize)]
struct SignBlobResponse {
    #[serde(rename = "signedBlob")]
    signed_blob: String,
}

pub(crate) fn signed_lifetime_seconds(expires_in: Option<Duration>) -> Result<u64, Exception> {
    let Some(expires_in) = expires_in else {
        return Err(exception!(
            code = "GCS_CONFIG",
            "signed URL lifetime is missing",
        ));
    };
    let seconds = expires_in.as_secs();
    if seconds == 0 {
        return Err(exception!(
            code = "GCS_CONFIG",
            "signed URL lifetime must be at least 1 second",
        ));
    }
    if seconds > MAX_SIGNED_URL_SECONDS {
        return Err(exception!(
            code = "GCS_CONFIG",
            "signed URL lifetime must be at most 604800 seconds",
        ));
    }
    Ok(seconds)
}

pub(crate) fn attachment_disposition(filename: &str) -> Result<String, Exception> {
    if filename.is_empty() {
        return Err(exception!(
            code = "GCS_CONFIG",
            "attachment filename is empty",
        ));
    }
    let fallback: String = filename
        .chars()
        .map(|ch| {
            if ch.is_ascii() && !ch.is_ascii_control() && ch != '"' && ch != '\\' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    Ok(format!(
        "attachment; filename=\"{fallback}\"; filename*=UTF-8''{}",
        encode_seg(filename)
    ))
}

pub(crate) fn encode_object_path(name: &str) -> String {
    name.split('/')
        .map(encode_seg)
        .collect::<Vec<_>>()
        .join("/")
}
