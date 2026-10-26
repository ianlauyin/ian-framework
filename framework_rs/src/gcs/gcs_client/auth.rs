use std::env;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::gcs::encode::encode_seg;
use crate::gcs::error::{api_detail, truncate};
use crate::gcs::{GcsError, GcsErrorKind};

const METADATA_ROOT: &str =
    "http://169.254.169.254/computeMetadata/v1/instance/service-accounts/default";
const CLOUD_PLATFORM_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const METADATA_TIMEOUT: Duration = Duration::from_secs(2);
const TOKEN_HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const TOKEN_SKEW: Duration = Duration::from_secs(60);
const ADC_FILE_LIMIT: u64 = 1024 * 1024;

#[derive(Clone)]
pub(super) enum Auth {
    Metadata,

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

impl Auth {
    pub(super) async fn from_adc() -> Result<Self, GcsError> {
        if let Ok(path) = env::var("GOOGLE_APPLICATION_CREDENTIALS") {
            if !path.is_empty() {
                return read_adc_at(Path::new(&path)).await;
            }
        }
        if let Some(path) = well_known_adc() {
            return read_adc_at(&path).await;
        }
        Ok(Self::Metadata)
    }

    pub(super) fn validate(&self) -> Result<(), GcsError> {
        let Auth::User {
            client_id,
            client_secret,
            refresh_token,
            signer_email,
        } = self
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

    pub(super) async fn signer_email(&self, http: &reqwest::Client) -> Result<String, GcsError> {
        match self {
            Auth::Metadata => metadata_email(http).await,
            Auth::User { signer_email, .. } => Ok(signer_email.clone()),
        }
    }

    pub(super) async fn fetch_token(
        &self,
        http: &reqwest::Client,
    ) -> Result<CachedToken, GcsError> {
        match self {
            Auth::Metadata => metadata_token(http).await,
            Auth::User {
                client_id,
                client_secret,
                refresh_token,
                ..
            } => user_token(http, client_id, client_secret, refresh_token).await,
        }
    }
}

pub(super) struct CachedToken {
    pub(super) value: String,
    pub(super) expires_at: Instant,
}

async fn metadata_email(http: &reqwest::Client) -> Result<String, GcsError> {
    let response = metadata_get(http, &format!("{METADATA_ROOT}/email")).await?;
    let status = response.status();
    let body = response.text().await.map_err(|err| {
        GcsError::with_source(
            GcsErrorKind::Auth,
            "metadata server email response is not text",
            err,
        )
    })?;
    if !status.is_success() {
        return Err(GcsError::new(
            GcsErrorKind::Auth,
            format!(
                "metadata server email status {}: {}",
                status.as_u16(),
                truncate(&body, 300)
            ),
        ));
    }
    let email = body.trim().to_string();
    validate_signer_email(&email)?;
    Ok(email)
}

async fn metadata_token(http: &reqwest::Client) -> Result<CachedToken, GcsError> {
    let url = format!(
        "{METADATA_ROOT}/token?scopes={}",
        encode_seg(CLOUD_PLATFORM_SCOPE),
    );
    let response = metadata_get(http, &url).await?;
    token_from_response(response).await
}

async fn user_token(
    http: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<CachedToken, GcsError> {
    let response = http
        .post(TOKEN_URL)
        .timeout(TOKEN_HTTP_TIMEOUT)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .map_err(|err| GcsError::with_source(GcsErrorKind::Auth, "token refresh failed", err))?;
    token_from_response(response).await
}

async fn metadata_get(http: &reqwest::Client, url: &str) -> Result<reqwest::Response, GcsError> {
    http.get(url)
        .header("Metadata-Flavor", "Google")
        .timeout(METADATA_TIMEOUT)
        .send()
        .await
        .map_err(|err| {
            GcsError::with_source(GcsErrorKind::Auth, "metadata server request failed", err)
        })
}

async fn token_from_response(response: reqwest::Response) -> Result<CachedToken, GcsError> {
    let status = response.status();
    let body = response.text().await.map_err(|err| {
        GcsError::with_source(GcsErrorKind::Auth, "token response is not text", err)
    })?;
    if !status.is_success() {
        let detail = api_detail(&body).unwrap_or(body);
        return Err(GcsError::new(
            GcsErrorKind::Auth,
            format!(
                "token status {}: {}",
                status.as_u16(),
                truncate(&detail, 300)
            ),
        ));
    }
    let parsed: TokenResponse = serde_json::from_str(&body).map_err(|err| {
        GcsError::with_source(GcsErrorKind::Auth, "token response is not JSON", err)
    })?;
    if parsed.access_token.is_empty() {
        return Err(GcsError::new(
            GcsErrorKind::Auth,
            "token response has an empty access token",
        ));
    }
    Ok(CachedToken {
        value: parsed.access_token,
        expires_at: expiry(parsed.expires_in),
    })
}

fn validate_signer_email(email: &str) -> Result<(), GcsError> {
    if !email.contains('@') || email.contains(char::is_whitespace) {
        return Err(GcsError::new(
            GcsErrorKind::Config,
            "signer email is not a service account email",
        ));
    }
    Ok(())
}

fn expiry(expires_in: u64) -> Instant {
    let lifetime = Duration::from_secs(expires_in);
    let lifetime = lifetime.saturating_sub(TOKEN_SKEW);
    Instant::now() + lifetime
}

#[derive(serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: u64,
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
    auth_from_value(&value, signer_email_from_env().as_deref())
}

fn signer_email_from_env() -> Option<String> {
    let value = env::var("GCS_SIGNER_EMAIL").ok()?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_string())
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
    // GcsClient::new checks that the fields are not empty and the signer email is valid.
    Ok(Auth::User {
        client_id: string_field(value, "client_id")?,
        client_secret: string_field(value, "client_secret")?,
        refresh_token: string_field(value, "refresh_token")?,
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

    fn auth_from_json_bytes(bytes: &[u8], signer_email: Option<&str>) -> Result<Auth, GcsError> {
        let value: Value = serde_json::from_slice(bytes).map_err(|err| {
            GcsError::with_source(
                GcsErrorKind::Config,
                "application default credentials are not JSON",
                err,
            )
        })?;
        auth_from_value(&value, signer_email)
    }

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
