use std::env;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::sync::Mutex;

use crate::exception;
use crate::exception::Exception;
use crate::gcp::{google_error_message, read_limited, truncate};

pub(crate) const METADATA_TOKEN_URL: &str =
    "http://169.254.169.254/computeMetadata/v1/instance/service-accounts/default/token";
pub(crate) const OAUTH_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
// The `cloud-platform` scope URL, percent-encoded for the query string.
const CLOUD_PLATFORM_SCOPE: &str = "https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcloud-platform";
const METADATA_TIMEOUT: Duration = Duration::from_secs(2);
const TOKEN_HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const TOKEN_SKEW: Duration = Duration::from_secs(60);
// Google gives this lifetime to a token when the response has no `expires_in`.
pub(crate) const DEFAULT_TOKEN_LIFETIME: Duration = Duration::from_secs(3600);
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
                code = "GCP_CONFIG",
                "user credentials are incomplete",
            ));
        }
        Ok(())
    }

    pub(crate) async fn fetch_token(
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
                    code = "GCP_AUTH",
                    "token refresh failed",
                    source = err.without_url(),
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

// The credentials and one cached access token. Each user gives its own HTTP client.
pub(crate) struct TokenSource {
    pub(crate) credentials: Credentials,
    pub(crate) metadata_token_url: String,
    pub(crate) oauth_token_url: String,
    pub(crate) cached: Mutex<Option<CachedToken>>,
}

impl TokenSource {
    pub(crate) fn new(credentials: Credentials) -> Self {
        Self {
            credentials,
            metadata_token_url: METADATA_TOKEN_URL.to_string(),
            oauth_token_url: OAUTH_TOKEN_URL.to_string(),
            cached: Mutex::new(None),
        }
    }

    // The lock stays held during a fetch, so concurrent calls send one token request.
    pub(crate) async fn token(&self, http: &reqwest::Client) -> Result<String, Exception> {
        let mut cached = self.cached.lock().await;
        if let Some(token) = cached.as_ref() {
            if Instant::now() < token.expires_at {
                return Ok(token.value.clone());
            }
        }
        let token = match &self.credentials {
            Credentials::Metadata => metadata_token(http, &self.metadata_token_url).await?,
            Credentials::User(user) => user.fetch_token(http, &self.oauth_token_url).await?,
        };
        let value = token.value.clone();
        *cached = Some(token);
        Ok(value)
    }

    pub(crate) async fn invalidate(&self) {
        *self.cached.lock().await = None;
    }
}

impl std::fmt::Debug for TokenSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenSource")
            .field("credentials", &self.credentials)
            .finish_non_exhaustive()
    }
}

pub(crate) async fn metadata_token(
    http: &reqwest::Client,
    token_url: &str,
) -> Result<CachedToken, Exception> {
    let url = format!("{token_url}?scopes={CLOUD_PLATFORM_SCOPE}");
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
                code = "GCP_AUTH",
                "metadata server request failed",
                source = err.without_url(),
            )
        })
}

pub(crate) async fn token_from_response(
    response: reqwest::Response,
) -> Result<CachedToken, Exception> {
    let status = response.status();
    let body = read_limited(response, "GCP_AUTH", "token response").await?;
    if !status.is_success() {
        let text = String::from_utf8_lossy(&body);
        let detail = google_error_message(&text).unwrap_or_else(|| text.to_string());
        return Err(exception!(
            code = "GCP_AUTH",
            format!(
                "token status {}: {}",
                status.as_u16(),
                truncate(&detail, 300)
            ),
        ));
    }
    let parsed: TokenResponse = serde_json::from_slice(&body).map_err(|err| {
        exception!(
            code = "GCP_AUTH",
            "token response is not JSON",
            source = err,
        )
    })?;
    if parsed.access_token.is_empty() {
        return Err(exception!(
            code = "GCP_AUTH",
            "token response has an empty access token",
        ));
    }
    Ok(CachedToken {
        value: parsed.access_token,
        expires_at: expiry(parsed.expires_in),
    })
}

pub(crate) fn expiry(expires_in: Option<u64>) -> Instant {
    let lifetime = expires_in
        .map_or(DEFAULT_TOKEN_LIFETIME, Duration::from_secs)
        .min(DEFAULT_TOKEN_LIFETIME);
    // A short lifetime keeps half of its time, so each call does not fetch a new token.
    let lifetime = lifetime.saturating_sub(TOKEN_SKEW).max(lifetime / 2);
    let now = Instant::now();
    now.checked_add(lifetime).unwrap_or(now)
}

pub(crate) async fn read_adc_at(path: &Path) -> Result<Credentials, Exception> {
    let meta = tokio::fs::metadata(path).await.map_err(|err| {
        exception!(
            code = "GCP_CONFIG",
            format!(
                "cannot read application default credentials at {}",
                path.display()
            ),
            source = err,
        )
    })?;
    if meta.len() > ADC_FILE_LIMIT {
        return Err(exception!(
            code = "GCP_CONFIG",
            "application default credentials file is too large",
        ));
    }
    let bytes = tokio::fs::read(path).await.map_err(|err| {
        exception!(
            code = "GCP_CONFIG",
            format!(
                "cannot read application default credentials at {}",
                path.display()
            ),
            source = err,
        )
    })?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|err| {
        exception!(
            code = "GCP_CONFIG",
            "application default credentials are not JSON",
            source = err,
        )
    })?;
    credentials_from_value(&value)
}

pub(crate) fn credentials_from_value(value: &Value) -> Result<Credentials, Exception> {
    let typ = value.get("type").and_then(Value::as_str).unwrap_or("");
    match typ {
        "service_account" => Err(exception!(
            code = "GCP_CONFIG",
            "service account JSON keys are rejected",
        )),
        "impersonated_service_account" => Err(exception!(
            code = "GCP_CONFIG",
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
        other => Err(exception!(
            code = "GCP_CONFIG",
            format!("application default credentials type `{other}` is not supported"),
        )),
    }
}

#[derive(serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
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

fn string_field(value: &Value, name: &str) -> Result<String, Exception> {
    value
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            exception!(
                code = "GCP_CONFIG",
                format!("user credentials are missing `{name}`"),
            )
        })
}
