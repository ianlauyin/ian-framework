use std::env;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::exception;
use crate::exception::Exception;
use crate::gcp::{api_detail, truncate};

pub(crate) const METADATA_TOKEN_URL: &str =
    "http://169.254.169.254/computeMetadata/v1/instance/service-accounts/default/token";
const CLOUD_PLATFORM_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const METADATA_TIMEOUT: Duration = Duration::from_secs(2);
const TOKEN_HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const TOKEN_SKEW: Duration = Duration::from_secs(60);
pub(crate) const ADC_FILE_LIMIT: u64 = 1024 * 1024;

#[derive(Clone)]
pub(crate) enum Credentials {
    Metadata,
    User(UserCredentials),
}

impl Credentials {
    pub(crate) async fn from_adc() -> Result<Self, Exception> {
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

    pub(crate) fn validate(&self) -> Result<(), Exception> {
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
    ) -> Result<CachedToken, Exception> {
        if let Credentials::User(user) = self {
            return user.fetch_token(http).await;
        }
        metadata_token(http, METADATA_TOKEN_URL).await
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
    pub(crate) fn validate(&self) -> Result<(), Exception> {
        if self.client_id.is_empty()
            || self.client_secret.is_empty()
            || self.refresh_token.is_empty()
        {
            return Err(exception!(
                "user credentials are incomplete",
                code = "GCP_CONFIG"
            ));
        }
        Ok(())
    }

    pub(crate) async fn fetch_token(
        &self,
        http: &reqwest::Client,
    ) -> Result<CachedToken, Exception> {
        self.fetch_token_at(http, TOKEN_URL).await
    }

    pub(crate) async fn fetch_token_at(
        &self,
        http: &reqwest::Client,
        url: &str,
    ) -> Result<CachedToken, Exception> {
        let response = http
            .post(url)
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
                exception!(
                    "token refresh failed",
                    code = "GCP_AUTH",
                    source = err.without_url()
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

pub(crate) async fn metadata_token(
    http: &reqwest::Client,
    token_url: &str,
) -> Result<CachedToken, Exception> {
    let scope: String = form_urlencoded::byte_serialize(CLOUD_PLATFORM_SCOPE.as_bytes()).collect();
    let url = format!("{token_url}?scopes={scope}");
    let response = metadata_get(http, &url).await?;
    token_from_response(response).await
}

pub(crate) async fn metadata_get(
    http: &reqwest::Client,
    url: &str,
) -> Result<reqwest::Response, Exception> {
    http.get(url)
        .header("Metadata-Flavor", "Google")
        .timeout(METADATA_TIMEOUT)
        .send()
        .await
        .map_err(|err| {
            exception!(
                "metadata server request failed",
                code = "GCP_AUTH",
                source = err.without_url()
            )
        })
}

pub(crate) async fn token_from_response(
    response: reqwest::Response,
) -> Result<CachedToken, Exception> {
    let status = response.status();
    let body = response.text().await.map_err(|err| {
        exception!(
            "token response is not text",
            code = "GCP_AUTH",
            source = err.without_url()
        )
    })?;
    if !status.is_success() {
        let detail = api_detail(&body).unwrap_or(body);
        return Err(exception!(
            format!(
                "token status {}: {}",
                status.as_u16(),
                truncate(&detail, 300)
            ),
            code = "GCP_AUTH"
        ));
    }
    let parsed: TokenResponse = serde_json::from_str(&body).map_err(|err| {
        exception!(
            "token response is not JSON",
            code = "GCP_AUTH",
            source = err
        )
    })?;
    if parsed.access_token.is_empty() {
        return Err(exception!(
            "token response has an empty access token",
            code = "GCP_AUTH"
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

pub(crate) async fn read_adc_at(path: &Path) -> Result<Credentials, Exception> {
    let meta = tokio::fs::metadata(path).await.map_err(|err| {
        exception!(
            format!(
                "cannot read application default credentials at {}",
                path.display()
            ),
            code = "GCP_CONFIG",
            source = err
        )
    })?;
    if meta.len() > ADC_FILE_LIMIT {
        return Err(exception!(
            "application default credentials file is too large",
            code = "GCP_CONFIG"
        ));
    }
    let bytes = tokio::fs::read(path).await.map_err(|err| {
        exception!(
            format!(
                "cannot read application default credentials at {}",
                path.display()
            ),
            code = "GCP_CONFIG",
            source = err
        )
    })?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|err| {
        exception!(
            "application default credentials are not JSON",
            code = "GCP_CONFIG",
            source = err
        )
    })?;
    credentials_from_value(&value)
}

pub(crate) fn credentials_from_value(value: &Value) -> Result<Credentials, Exception> {
    let typ = value.get("type").and_then(Value::as_str).unwrap_or("");
    match typ {
        "service_account" => Err(exception!(
            "service account JSON keys are rejected",
            code = "GCP_CONFIG"
        )),
        "impersonated_service_account" => Err(exception!(
            "impersonated application default credentials are rejected",
            code = "GCP_CONFIG"
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
        other => Err(exception!(
            format!("application default credentials type `{other}` is not supported"),
            code = "GCP_CONFIG"
        )),
    }
}

fn string_field(value: &Value, name: &str) -> Result<String, Exception> {
    value
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            exception!(
                format!("user credentials are missing `{name}`"),
                code = "GCP_CONFIG"
            )
        })
}
