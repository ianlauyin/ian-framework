use std::env;
use std::time::Duration;

use tokio::sync::OnceCell;

use crate::exception;
use crate::exception::Exception;
use crate::gcp::{
    http_client_builder, is_retryable_status, metadata_get, read_limited, Credentials, TokenSource,
};

pub(crate) mod access_response;

const API_ROOT: &str = "https://secretmanager.googleapis.com/v1";
const METADATA_PROJECT_URL: &str = "http://169.254.169.254/computeMetadata/v1/project/project-id";
pub(crate) const PROJECT_ENV: &str = "GOOGLE_CLOUD_PROJECT";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const ACCESS_ATTEMPTS: u32 = 3;
const ACCESS_BACKOFF: Duration = Duration::from_millis(250);
// None means that the process has no credentials.
pub(crate) static SECRET_MANAGER: OnceCell<Option<SecretManager>> = OnceCell::const_new();

// The value is a secret. No error of this file holds the value or a parser error that can quote it.
pub struct SecretManager {
    pub(crate) http: reqwest::Client,
    pub(crate) api_root: String,
    pub(crate) project: String,
    pub(crate) quota_project: Option<String>,
    pub(crate) tokens: TokenSource,
}

impl SecretManager {
    pub async fn shared() -> Result<Option<&'static SecretManager>, Exception> {
        let state = SECRET_MANAGER.get_or_try_init(Self::from_adc).await?;
        Ok(state.as_ref())
    }

    // None means that the secret or its latest version does not exist.
    pub async fn access_latest(&self, secret: &str) -> Result<Option<String>, Exception> {
        if !is_name_segment(secret) {
            return Err(exception!(
                code = "GCP_CONFIG",
                "secret id has a character that is not valid",
            ));
        }
        let url = format!(
            "{}/projects/{}/secrets/{secret}/versions/latest:access",
            self.api_root, self.project
        );
        let mut refreshed = false;
        let mut attempts = 0u32;
        loop {
            let token = self.tokens.token(&self.http).await?;
            let reply = access_at(&self.http, &url, &token, self.quota_project.as_deref()).await;
            let retry = match &reply {
                Ok((401, _)) if !refreshed => {
                    refreshed = true;
                    self.tokens.invalidate().await;
                    continue;
                }
                Ok((status, _)) => is_retryable_status(*status),
                Err(_) => true,
            };
            if !retry || attempts + 1 >= ACCESS_ATTEMPTS {
                let (status, body) = reply?;
                return access_response::decode_access_response(status, &body);
            }
            attempts += 1;
            tokio::time::sleep(ACCESS_BACKOFF * attempts).await;
        }
    }

    pub(crate) async fn from_adc() -> Result<Option<Self>, Exception> {
        let credentials = Credentials::from_adc().await?;
        credentials.validate()?;
        let http = http_client_builder().build().map_err(|err| {
            exception!(
                code = "GCP_TRANSPORT",
                "cannot build the HTTP client",
                source = err,
            )
        })?;
        let Some(project) = resolve_project(&credentials, &http, METADATA_PROJECT_URL).await?
        else {
            return Ok(None);
        };
        let quota_project = credentials.quota_project().map(str::to_string);
        let tokens = TokenSource::new(credentials);
        tokens.token(&http).await?;
        Ok(Some(Self {
            http,
            api_root: API_ROOT.to_string(),
            project,
            quota_project,
            tokens,
        }))
    }
}

pub(crate) async fn resolve_project(
    credentials: &Credentials,
    http: &reqwest::Client,
    metadata_project_url: &str,
) -> Result<Option<String>, Exception> {
    match credentials {
        Credentials::Metadata => metadata_project(http, metadata_project_url).await.map(Some),
        Credentials::User(_) => user_project(credentials),
    }
}

pub(crate) async fn metadata_project(
    http: &reqwest::Client,
    url: &str,
) -> Result<String, Exception> {
    let response = metadata_get(http, url).await?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(exception!(
            code = "GCP_AUTH",
            format!("metadata server project status {status}"),
        ));
    }
    let body = read_limited(response, "GCP_TRANSPORT", "metadata server project").await?;
    checked_project(String::from_utf8_lossy(&body).trim())
}

pub(crate) fn user_project(credentials: &Credentials) -> Result<Option<String>, Exception> {
    let from_env = env::var(PROJECT_ENV)
        .ok()
        .filter(|project| !project.trim().is_empty());
    match from_env.as_deref().or(credentials.quota_project()) {
        Some(project) => checked_project(project.trim()).map(Some),
        None => Ok(None),
    }
}

pub(crate) async fn access_at(
    http: &reqwest::Client,
    url: &str,
    token: &str,
    quota_project: Option<&str>,
) -> Result<(u16, Vec<u8>), Exception> {
    let mut request = http.get(url).bearer_auth(token).timeout(REQUEST_TIMEOUT);
    if let Some(project) = quota_project {
        request = request.header("x-goog-user-project", project);
    }
    let response = request.send().await.map_err(|err| {
        exception!(
            code = "GCP_TRANSPORT",
            "secret manager request failed",
            source = err.without_url(),
        )
    })?;
    let status = response.status().as_u16();
    let body = read_limited(response, "GCP_TRANSPORT", "secret manager response").await?;
    Ok((status, body))
}

fn checked_project(project: &str) -> Result<String, Exception> {
    if !is_name_segment(project) {
        return Err(exception!(
            code = "GCP_CONFIG",
            "the project id is empty or not valid",
        ));
    }
    Ok(project.to_string())
}

fn is_name_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}
