use std::time::Duration;

use google_cloud_auth::credentials;
use google_cloud_auth::signer::Signer;

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

pub(crate) fn percent_encode(segment: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push('%');
                out.push(HEX[(byte >> 4) as usize] as char);
                out.push(HEX[(byte & 0x0f) as usize] as char);
            }
        }
    }
    out
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
