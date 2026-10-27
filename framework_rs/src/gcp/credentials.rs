use std::env;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::gcp::{api_detail, truncate, GcpError, GcpErrorKind};

const METADATA_TOKEN_URL: &str =
    "http://169.254.169.254/computeMetadata/v1/instance/service-accounts/default/token";
const CLOUD_PLATFORM_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const METADATA_TIMEOUT: Duration = Duration::from_secs(2);
const TOKEN_HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const TOKEN_SKEW: Duration = Duration::from_secs(60);
const ADC_FILE_LIMIT: u64 = 1024 * 1024;

#[derive(Clone)]
pub(crate) enum Credentials {
    Metadata,
    User(UserCredentials),
}

impl Credentials {
    pub(crate) async fn from_adc() -> Result<Self, GcpError> {
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

    pub(crate) fn validate(&self) -> Result<(), GcpError> {
        match self {
            Credentials::Metadata => Ok(()),
            Credentials::User(user) => user.validate(),
        }
    }

    pub(crate) fn quota_project(&self) -> Option<&str> {
        match self {
            Credentials::Metadata => None,
            Credentials::User(user) => user.quota_project_id.as_deref(),
        }
    }

    pub(crate) async fn fetch_token(
        &self,
        http: &reqwest::Client,
    ) -> Result<CachedToken, GcpError> {
        match self {
            Credentials::Metadata => metadata_token(http).await,
            Credentials::User(user) => user.fetch_token(http).await,
        }
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Credentials::Metadata => f.write_str("Metadata"),
            Credentials::User(user) => f.debug_tuple("User").field(user).finish(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct UserCredentials {
    pub(crate) client_id: String,
    pub(crate) client_secret: String,
    pub(crate) refresh_token: String,
    pub(crate) quota_project_id: Option<String>,
}

impl UserCredentials {
    pub(crate) fn validate(&self) -> Result<(), GcpError> {
        if self.client_id.is_empty()
            || self.client_secret.is_empty()
            || self.refresh_token.is_empty()
        {
            return Err(GcpError::new(
                GcpErrorKind::Config,
                "user credentials are incomplete",
            ));
        }
        Ok(())
    }

    pub(crate) async fn fetch_token(
        &self,
        http: &reqwest::Client,
    ) -> Result<CachedToken, GcpError> {
        let response = http
            .post(TOKEN_URL)
            .timeout(TOKEN_HTTP_TIMEOUT)
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("refresh_token", self.refresh_token.as_str()),
            ])
            .send()
            .await
            .map_err(|err| {
                GcpError::with_source(
                    GcpErrorKind::Auth,
                    "token refresh failed",
                    err.without_url(),
                )
            })?;
        token_from_response(response).await
    }
}

impl std::fmt::Debug for UserCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserCredentials")
            .field("quota_project_id", &self.quota_project_id)
            .finish_non_exhaustive()
    }
}

pub(crate) struct CachedToken {
    pub(crate) value: String,
    pub(crate) expires_at: Instant,
}

pub(crate) async fn metadata_token(http: &reqwest::Client) -> Result<CachedToken, GcpError> {
    let scope: String = form_urlencoded::byte_serialize(CLOUD_PLATFORM_SCOPE.as_bytes()).collect();
    let url = format!("{METADATA_TOKEN_URL}?scopes={scope}");
    let response = metadata_get(http, &url).await?;
    token_from_response(response).await
}

pub(crate) async fn metadata_get(
    http: &reqwest::Client,
    url: &str,
) -> Result<reqwest::Response, GcpError> {
    http.get(url)
        .header("Metadata-Flavor", "Google")
        .timeout(METADATA_TIMEOUT)
        .send()
        .await
        .map_err(|err| {
            GcpError::with_source(
                GcpErrorKind::Auth,
                "metadata server request failed",
                err.without_url(),
            )
        })
}

async fn token_from_response(response: reqwest::Response) -> Result<CachedToken, GcpError> {
    let status = response.status();
    let body = response.text().await.map_err(|err| {
        GcpError::with_source(
            GcpErrorKind::Auth,
            "token response is not text",
            err.without_url(),
        )
    })?;
    if !status.is_success() {
        let detail = api_detail(&body).unwrap_or(body);
        return Err(GcpError::new(
            GcpErrorKind::Auth,
            format!(
                "token status {}: {}",
                status.as_u16(),
                truncate(&detail, 300)
            ),
        ));
    }
    let parsed: TokenResponse = serde_json::from_str(&body).map_err(|err| {
        GcpError::with_source(GcpErrorKind::Auth, "token response is not JSON", err)
    })?;
    if parsed.access_token.is_empty() {
        return Err(GcpError::new(
            GcpErrorKind::Auth,
            "token response has an empty access token",
        ));
    }
    Ok(CachedToken {
        value: parsed.access_token,
        expires_at: expiry(parsed.expires_in),
    })
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

async fn read_adc_at(path: &Path) -> Result<Credentials, GcpError> {
    let meta = tokio::fs::metadata(path).await.map_err(|err| {
        GcpError::with_source(
            GcpErrorKind::Config,
            format!(
                "cannot read application default credentials at {}",
                path.display()
            ),
            err,
        )
    })?;
    if meta.len() > ADC_FILE_LIMIT {
        return Err(GcpError::new(
            GcpErrorKind::Config,
            "application default credentials file is too large",
        ));
    }
    let bytes = tokio::fs::read(path).await.map_err(|err| {
        GcpError::with_source(
            GcpErrorKind::Config,
            format!(
                "cannot read application default credentials at {}",
                path.display()
            ),
            err,
        )
    })?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|err| {
        GcpError::with_source(
            GcpErrorKind::Config,
            "application default credentials are not JSON",
            err,
        )
    })?;
    credentials_from_value(&value)
}

fn credentials_from_value(value: &Value) -> Result<Credentials, GcpError> {
    let typ = value.get("type").and_then(Value::as_str).unwrap_or("");
    match typ {
        "service_account" => Err(GcpError::new(
            GcpErrorKind::Config,
            "service account JSON keys are rejected",
        )),
        "impersonated_service_account" => Err(GcpError::new(
            GcpErrorKind::Config,
            "impersonated application default credentials are rejected",
        )),
        "authorized_user" => Ok(Credentials::User(UserCredentials {
            client_id: string_field(value, "client_id")?,
            client_secret: string_field(value, "client_secret")?,
            refresh_token: string_field(value, "refresh_token")?,
            quota_project_id: value
                .get("quota_project_id")
                .and_then(Value::as_str)
                .filter(|project| !project.trim().is_empty())
                .map(|project| project.trim().to_string()),
        })),
        other => Err(GcpError::new(
            GcpErrorKind::Config,
            format!("application default credentials type `{other}` is not supported"),
        )),
    }
}

fn string_field(value: &Value, name: &str) -> Result<String, GcpError> {
    value
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            GcpError::new(
                GcpErrorKind::Config,
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

    fn credentials_from_json(json: &str) -> Result<Credentials, GcpError> {
        credentials_from_value(&serde_json::from_str(json).unwrap())
    }

    #[test]
    fn service_account_key_is_rejected_without_echoing_the_key() {
        let json = r#"{
            "type": "service_account",
            "client_email": "app@example.iam.gserviceaccount.com",
            "private_key": "SECRET-KEY-MATERIAL"
        }"#;
        let err = credentials_from_json(json).unwrap_err();
        assert_eq!(err.kind(), GcpErrorKind::Config);
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
        let err = credentials_from_json(json).unwrap_err();
        assert_eq!(err.kind(), GcpErrorKind::Config);
        assert!(err.to_string().contains("impersonated"));
        assert!(!err.to_string().contains("generateAccessToken"));
    }

    #[test]
    fn external_account_is_rejected() {
        let json = r#"{"type":"external_account","audience":"x"}"#;
        let err = credentials_from_json(json).unwrap_err();
        assert!(err.to_string().contains("external_account"));
    }

    #[test]
    fn user_adc_reads_the_fields() {
        let Credentials::User(user) = credentials_from_json(USER_JSON).unwrap() else {
            panic!("expected user credentials");
        };
        assert_eq!(user.client_id, "id.apps.googleusercontent.com");
        assert_eq!(user.client_secret, "secret");
        assert_eq!(user.refresh_token, "refresh");
        assert_eq!(user.quota_project_id, None);
        assert!(user.validate().is_ok());
    }

    #[test]
    fn user_adc_keeps_the_quota_project() {
        let json = r#"{
            "type": "authorized_user",
            "client_id": "id",
            "client_secret": "secret",
            "refresh_token": "refresh",
            "quota_project_id": "my-project"
        }"#;
        let Credentials::User(user) = credentials_from_json(json).unwrap() else {
            panic!("expected user credentials");
        };
        assert_eq!(user.quota_project_id.as_deref(), Some("my-project"));
    }

    #[test]
    fn user_adc_needs_each_field() {
        let json = r#"{"type":"authorized_user","client_id":"id","client_secret":"secret"}"#;
        let err = credentials_from_json(json).unwrap_err();
        assert_eq!(err.kind(), GcpErrorKind::Config);
        assert!(err.to_string().contains("refresh_token"));
    }

    #[test]
    fn incomplete_user_is_rejected() {
        let user = UserCredentials {
            client_id: String::new(),
            client_secret: "secret".to_string(),
            refresh_token: "refresh".to_string(),
            quota_project_id: None,
        };
        assert_eq!(user.validate().unwrap_err().kind(), GcpErrorKind::Config);
    }

    #[test]
    fn debug_does_not_print_the_secrets() {
        let credentials = credentials_from_json(USER_JSON).unwrap();
        let text = format!("{credentials:?}");
        assert!(text.contains("User"));
        assert!(!text.contains("refresh"));
        assert!(!text.contains("secret"));
        assert!(!text.contains("googleusercontent"));
    }
}
