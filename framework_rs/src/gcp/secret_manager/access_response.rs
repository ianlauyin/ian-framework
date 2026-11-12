use base64::Engine;

use crate::exception;
use crate::exception::Exception;
use crate::gcp::{google_error_message, truncate};

#[derive(serde::Deserialize)]
struct AccessResponse {
    payload: Payload,
}

#[derive(serde::Deserialize)]
struct Payload {
    data: String,
}

pub(crate) fn decode_access_response(
    status: u16,
    body: &[u8],
) -> Result<Option<String>, Exception> {
    if status == 404 {
        return Ok(None);
    }
    if !(200..300).contains(&status) {
        let detail = google_error_message(&String::from_utf8_lossy(body)).unwrap_or_default();
        let kind = match status {
            401 | 403 => "GCP_AUTH",
            _ => "GCP_API",
        };
        return Err(exception!(
            code = kind,
            format!("secret manager status {status}: {}", truncate(&detail, 300)),
        ));
    }
    decode_payload(body).map(Some)
}

fn decode_payload(body: &[u8]) -> Result<String, Exception> {
    let parsed: AccessResponse = serde_json::from_slice(body).map_err(|_| {
        exception!(
            code = "GCP_API",
            "secret manager response has no payload data",
        )
    })?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(parsed.payload.data)
        .map_err(|_| exception!(code = "GCP_API", "secret payload is not base64"))?;
    let value = String::from_utf8(bytes)
        .map_err(|_| exception!(code = "GCP_CONFIG", "secret value is not UTF-8"))?;
    let value = value.trim();
    if value.is_empty() {
        return Err(exception!(code = "GCP_CONFIG", "secret value is empty"));
    }
    Ok(value.to_string())
}
