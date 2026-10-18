//! Application Default Credentials for the GCS client.
//!
//! Accepted:
//!
//! - The metadata server service account on a VM (`Auth::Metadata`).
//! - User ADC (`type` = `authorized_user`) when `GCS_SIGNER_EMAIL` is set.
//!
//! Rejected:
//!
//! - Service account JSON keys (`type` = `service_account`).
//! - Impersonated ADC (`type` = `impersonated_service_account`).
//! - Any other ADC type.

use std::env;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{GcsError, GcsErrorKind};

const ADC_FILE_LIMIT: u64 = 1024 * 1024;

/// How the client obtains an access token and a signer email.
#[derive(Clone)]
pub enum Auth {
    /// VM metadata server. The attached service account is the signer.
    Metadata,
    /// End-user ADC (`authorized_user`).
    ///
    /// `signer_email` is the service account that `signBlob` uses.
    /// The refresh token is not a service account key.
    User {
        client_id: String,
        client_secret: String,
        refresh_token: String,
        signer_email: String,
    },
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Auth::Metadata => f.write_str("Metadata"),
            Auth::User { signer_email, .. } => f
                .debug_struct("User")
                .field("signer_email", signer_email)
                .finish_non_exhaustive(),
        }
    }
}

pub(crate) fn validate_auth(auth: &Auth) -> Result<(), GcsError> {
    let Auth::User {
        client_id,
        client_secret,
        refresh_token,
        signer_email,
    } = auth
    else {
        return Ok(());
    };
    if client_id.is_empty() || client_secret.is_empty() || refresh_token.is_empty() {
        return Err(GcsError::new(
            GcsErrorKind::Config,
            "user credentials are incomplete",
        ));
    }
    validate_signer_email(signer_email)
}

pub(crate) fn validate_signer_email(email: &str) -> Result<(), GcsError> {
    if email.is_empty()
        || !email.contains('@')
        || email.contains(char::is_whitespace)
        || email.contains(['\r', '\n'])
    {
        return Err(GcsError::new(
            GcsErrorKind::Config,
            "signer email is not a service account email",
        ));
    }
    Ok(())
}

/// Load ADC from the environment.
///
/// `Ok(None)` means no credentials file is configured, so the caller uses the metadata server.
/// A set `GOOGLE_APPLICATION_CREDENTIALS` that does not point at a file is an error.
pub(crate) async fn load_adc_auth() -> Result<Option<Auth>, GcsError> {
    if let Ok(path) = env::var("GOOGLE_APPLICATION_CREDENTIALS") {
        if !path.is_empty() {
            return read_adc_at(Path::new(&path)).await.map(Some);
        }
    }
    if let Some(path) = well_known_adc() {
        return read_adc_at(&path).await.map(Some);
    }
    Ok(None)
}

fn well_known_adc() -> Option<PathBuf> {
    if let Some(home) = env::var_os("HOME") {
        let path = PathBuf::from(home).join(".config/gcloud/application_default_credentials.json");
        if path.is_file() {
            return Some(path);
        }
    }
    if let Some(appdata) = env::var_os("APPDATA") {
        let path = PathBuf::from(appdata).join("gcloud/application_default_credentials.json");
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

async fn read_adc_at(path: &Path) -> Result<Auth, GcsError> {
    let meta = tokio::fs::metadata(path).await.map_err(|err| {
        GcsError::with_source(
            GcsErrorKind::Config,
            format!(
                "cannot read application default credentials at {}",
                path.display()
            ),
            err,
        )
    })?;
    if meta.len() > ADC_FILE_LIMIT {
        return Err(GcsError::new(
            GcsErrorKind::Config,
            "application default credentials file is too large",
        ));
    }
    let bytes = tokio::fs::read(path).await.map_err(|err| {
        GcsError::with_source(
            GcsErrorKind::Config,
            format!(
                "cannot read application default credentials at {}",
                path.display()
            ),
            err,
        )
    })?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|err| {
        GcsError::with_source(
            GcsErrorKind::Config,
            "application default credentials are not JSON",
            err,
        )
    })?;
    let typ = value.get("type").and_then(Value::as_str).unwrap_or("");
    let signer = if typ == "authorized_user" {
        Some(signer_email_from_env()?)
    } else {
        None
    };
    auth_from_value(&value, signer.as_deref())
}

fn signer_email_from_env() -> Result<String, GcsError> {
    let value = env::var("GCS_SIGNER_EMAIL").unwrap_or_default();
    let value = value.trim();
    if value.is_empty() {
        return Err(GcsError::new(
            GcsErrorKind::Config,
            "user application default credentials require GCS_SIGNER_EMAIL",
        ));
    }
    validate_signer_email(value)?;
    Ok(value.to_string())
}

#[cfg(test)]
pub(crate) fn auth_from_json_bytes(
    bytes: &[u8],
    signer_email: Option<&str>,
) -> Result<Auth, GcsError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|err| {
        GcsError::with_source(
            GcsErrorKind::Config,
            "application default credentials are not JSON",
            err,
        )
    })?;
    auth_from_value(&value, signer_email)
}

fn auth_from_value(value: &Value, signer_email: Option<&str>) -> Result<Auth, GcsError> {
    let typ = value.get("type").and_then(Value::as_str).unwrap_or("");
    match typ {
        "service_account" => Err(GcsError::new(
            GcsErrorKind::Config,
            "service account JSON keys are rejected",
        )),
        "impersonated_service_account" => Err(GcsError::new(
            GcsErrorKind::Config,
            "impersonated application default credentials are rejected",
        )),
        "authorized_user" => authorized_user(value, signer_email),
        other => Err(GcsError::new(
            GcsErrorKind::Config,
            format!("application default credentials type `{other}` is not supported"),
        )),
    }
}

fn authorized_user(value: &Value, signer_email: Option<&str>) -> Result<Auth, GcsError> {
    let signer_email = signer_email.ok_or_else(|| {
        GcsError::new(
            GcsErrorKind::Config,
            "user application default credentials require GCS_SIGNER_EMAIL",
        )
    })?;
    validate_signer_email(signer_email)?;
    let client_id = string_field(value, "client_id")?;
    let client_secret = string_field(value, "client_secret")?;
    let refresh_token = string_field(value, "refresh_token")?;
    if client_id.is_empty() || client_secret.is_empty() || refresh_token.is_empty() {
        return Err(GcsError::new(
            GcsErrorKind::Config,
            "user credentials are incomplete",
        ));
    }
    Ok(Auth::User {
        client_id,
        client_secret,
        refresh_token,
        signer_email: signer_email.to_string(),
    })
}

fn string_field(value: &Value, name: &str) -> Result<String, GcsError> {
    value
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            GcsError::new(
                GcsErrorKind::Config,
                format!("user credentials are missing `{name}`"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER_JSON: &str = r#"{
        "type": "authorized_user",
        "client_id": "id.apps.googleusercontent.com",
        "client_secret": "secret",
        "refresh_token": "refresh"
    }"#;

    #[test]
    fn service_account_key_is_rejected_without_echoing_the_key() {
        let json = r#"{
            "type": "service_account",
            "client_email": "app@example.iam.gserviceaccount.com",
            "private_key": "SECRET-KEY-MATERIAL"
        }"#;
        let err = auth_from_json_bytes(json.as_bytes(), None).unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Config);
        let text = err.to_string();
        assert!(text.contains("service account JSON keys"));
        assert!(!text.contains("SECRET-KEY-MATERIAL"));
    }

    #[test]
    fn impersonated_adc_is_rejected() {
        let json = r#"{
            "type": "impersonated_service_account",
            "service_account_impersonation_url": "https://iamcredentials.googleapis.com/v1/projects/-/serviceAccounts/a@b.iam.gserviceaccount.com:generateAccessToken",
            "source_credentials": {"type": "authorized_user"}
        }"#;
        let err =
            auth_from_json_bytes(json.as_bytes(), Some("a@b.iam.gserviceaccount.com")).unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Config);
        assert!(err.to_string().contains("impersonated"));
        assert!(!err.to_string().contains("generateAccessToken"));
    }

    #[test]
    fn external_account_is_rejected() {
        let json = r#"{"type":"external_account","audience":"x"}"#;
        let err = auth_from_json_bytes(json.as_bytes(), None).unwrap_err();
        assert!(err.to_string().contains("external_account"));
    }

    #[test]
    fn user_adc_requires_a_signer_email() {
        let err = auth_from_json_bytes(USER_JSON.as_bytes(), None).unwrap_err();
        assert!(err.to_string().contains("GCS_SIGNER_EMAIL"));
    }

    #[test]
    fn user_adc_keeps_the_signer_email() {
        let auth = auth_from_json_bytes(
            USER_JSON.as_bytes(),
            Some("signer@example.iam.gserviceaccount.com"),
        )
        .unwrap();
        match auth {
            Auth::User {
                client_id,
                signer_email,
                ..
            } => {
                assert_eq!(client_id, "id.apps.googleusercontent.com");
                assert_eq!(signer_email, "signer@example.iam.gserviceaccount.com");
            }
            Auth::Metadata => panic!("expected user auth"),
        }
    }

    #[test]
    fn debug_does_not_print_the_refresh_token() {
        let auth = auth_from_json_bytes(
            USER_JSON.as_bytes(),
            Some("signer@example.iam.gserviceaccount.com"),
        )
        .unwrap();
        let text = format!("{auth:?}");
        assert!(text.contains("signer@example.iam.gserviceaccount.com"));
        assert!(!text.contains("refresh"));
        assert!(!text.contains("secret"));
    }
}
