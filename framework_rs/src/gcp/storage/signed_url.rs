use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use google_cloud_auth::credentials;
use google_cloud_auth::signer::Signer;
use sha2::Digest;
use sha2::Sha256;

use crate::exception;
use crate::exception::Exception;

pub const ATTACHMENT_FILENAME_EMPTY: &str = "ATTACHMENT_FILENAME_EMPTY";
pub const SIGNER_EMAIL_INVALID: &str = "SIGNER_EMAIL_INVALID";
pub const URL_SIGNER_BUILD_FAILED: &str = "URL_SIGNER_BUILD_FAILED";
pub const SIGNED_URL_LIFETIME_TOO_LONG: &str = "SIGNED_URL_LIFETIME_TOO_LONG";
pub const SIGNED_URL_LIFETIME_TOO_SHORT: &str = "SIGNED_URL_LIFETIME_TOO_SHORT";
const MAX_SIGNED_URL_SECONDS: u64 = 604_800;

pub(crate) fn validate_lifetime(expires_in: Duration) -> Result<Duration, Exception> {
    let seconds = expires_in.as_secs();
    if seconds == 0 {
        return Err(exception!(
            code = SIGNED_URL_LIFETIME_TOO_SHORT,
            "signed URL lifetime must be at least 1 second",
        ));
    }
    if seconds > MAX_SIGNED_URL_SECONDS {
        return Err(exception!(
            code = SIGNED_URL_LIFETIME_TOO_LONG,
            "signed URL lifetime must be at most 604800 seconds",
        ));
    }
    Ok(Duration::from_secs(seconds))
}

pub(crate) fn content_disposition_value(filename: &str) -> Result<String, Exception> {
    if filename.is_empty() {
        return Err(exception!(
            code = ATTACHMENT_FILENAME_EMPTY,
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
        percent_encode(filename)
    ))
}

pub(crate) struct SignedGet {
    pub string_to_sign: String,
    pub url: String,
}

pub(crate) fn signed_get(
    bucket: &str,
    object: &str,
    client_email: &str,
    expires_secs: u64,
    now: DateTime<Utc>,
    disposition: Option<&str>,
) -> SignedGet {
    let stamp = now.format("%Y%m%dT%H%M%SZ").to_string();
    let day = now.format("%Y%m%d").to_string();
    let scope = format!("{day}/auto/storage/goog4_request");
    let credential = format!("{client_email}/{scope}");
    let mut params = BTreeMap::new();
    params.insert("X-Goog-Algorithm", "GOOG4-RSA-SHA256".to_string());
    params.insert("X-Goog-Credential", credential);
    params.insert("X-Goog-Date", stamp.clone());
    params.insert("X-Goog-Expires", expires_secs.to_string());
    params.insert("X-Goog-SignedHeaders", "host".to_string());
    if let Some(disposition) = disposition {
        params.insert("response-content-disposition", disposition.to_string());
    }
    let query = canonical_query(&params);
    let path = format!("/{bucket}/{}", encode_path(object));
    let request =
        format!("GET\n{path}\n{query}\nhost:storage.googleapis.com\n\nhost\nUNSIGNED-PAYLOAD");
    let hash = hex::encode(Sha256::digest(request.as_bytes()));
    SignedGet {
        string_to_sign: format!("GOOG4-RSA-SHA256\n{stamp}\n{scope}\n{hash}"),
        url: format!("https://storage.googleapis.com{path}?{query}"),
    }
}

pub(crate) fn percent_encode(segment: &str) -> String {
    encode(segment, false)
}

pub(crate) fn encode_path(object: &str) -> String {
    encode(object, true)
}

pub(crate) fn build_signer(signer_email: Option<&str>) -> Result<Signer, Exception> {
    let build_failed = |err| {
        exception!(
            code = URL_SIGNER_BUILD_FAILED,
            "cannot build the URL signer",
            source = err,
        )
    };
    let Some(email) = signer_email else {
        return credentials::Builder::default()
            .build_signer()
            .map_err(build_failed);
    };
    if !email.contains('@') || email.contains(char::is_whitespace) {
        return Err(exception!(
            code = SIGNER_EMAIL_INVALID,
            "signer email is not a service account email",
        ));
    }
    let source = credentials::Builder::default()
        .build()
        .map_err(build_failed)?;
    credentials::impersonated::Builder::from_source_credentials(source)
        .with_target_principal(email)
        .build_signer()
        .map_err(build_failed)
}

fn canonical_query(params: &BTreeMap<&str, String>) -> String {
    params
        .iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn encode(segment: &str, keep_slash: bool) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            b'/' if keep_slash => out.push('/'),
            _ => {
                out.push('%');
                out.push(HEX[(byte >> 4) as usize] as char);
                out.push(HEX[(byte & 0x0f) as usize] as char);
            }
        }
    }
    out
}
