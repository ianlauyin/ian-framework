use std::env;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};

use base64::Engine;
use tokio::sync::OnceCell;

use crate::exception;
use crate::exception::Exception;
use crate::gcp::{
    google_error_message, http_client_builder, is_retryable_status, metadata_get, read_limited,
    truncate, Credentials, TokenSource,
};

const API_ROOT: &str = "https://secretmanager.googleapis.com/v1";
const METADATA_PROJECT_URL: &str = "http://169.254.169.254/computeMetadata/v1/project/project-id";
// Not an error for the caller. `initialized` maps it to no credentials and does not cache it.
pub(crate) const METADATA_UNREACHABLE: &str = "GCP_METADATA_UNREACHABLE";
pub(crate) const PROJECT_ENV: &str = "GOOGLE_CLOUD_PROJECT";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const ACCESS_ATTEMPTS: u32 = 3;
const ACCESS_BACKOFF: Duration = Duration::from_millis(250);
// After a failed metadata probe, later calls skip the probe for this time.
const UNREACHABLE_RETRY: Duration = Duration::from_secs(30);

// None means that the process has no credentials, so each read falls back to the env var.
pub(crate) static SECRET_MANAGER: OnceCell<Option<SecretManager>> = OnceCell::const_new();

// The metadata server may be down for a short time, so an unreachable server is not kept in the OnceCell.
static UNREACHABLE_AT: StdMutex<Option<Instant>> = StdMutex::new(None);

// The value is a secret. No error of this file holds the value or a parser error that can quote it.
pub(crate) struct SecretManager {
    pub(crate) http: reqwest::Client,
    pub(crate) api_root: String,
    pub(crate) project: String,
    pub(crate) quota_project: Option<String>,
    pub(crate) tokens: TokenSource,
}

impl SecretManager {
    pub(crate) async fn shared() -> Result<Option<&'static SecretManager>, Exception> {
        if let Some(state) = SECRET_MANAGER.get() {
            return Ok(state.as_ref());
        }
        if unreachable_recently() {
            return Ok(None);
        }
        initialized(SECRET_MANAGER.get_or_try_init(Self::from_adc).await)
    }

    // None means that the secret or its latest version does not exist.
    pub(crate) async fn access_latest(&self, secret: &str) -> Result<Option<String>, Exception> {
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
                return decode_access_response(status, &body);
            }
            attempts += 1;
            tokio::time::sleep(ACCESS_BACKOFF * attempts).await;
        }
    }

    pub(crate) async fn from_adc() -> Result<Option<Self>, Exception> {
        let credentials = Credentials::from_adc().await?;
        credentials.validate()?;
        let http = secret_manager_http_client()?;
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

pub(crate) fn initialized(
    result: Result<&'static Option<SecretManager>, Exception>,
) -> Result<Option<&'static SecretManager>, Exception> {
    match result {
        Ok(state) => Ok(state.as_ref()),
        Err(err) if err.code() == METADATA_UNREACHABLE => {
            *lock_unreachable() = Some(Instant::now());
            Ok(None)
        }
        Err(err) => Err(err),
    }
}

pub(crate) async fn resolve_project(
    credentials: &Credentials,
    http: &reqwest::Client,
    metadata_project_url: &str,
) -> Result<Option<String>, Exception> {
    match credentials {
        Credentials::Metadata => metadata_project(http, metadata_project_url)
            .await?
            .ok_or_else(|| {
                exception!(
                    code = METADATA_UNREACHABLE,
                    "metadata server is unreachable"
                )
            })
            .map(Some),
        Credentials::User(_) => Ok(user_project(credentials)?),
    }
}

pub(crate) fn lock_unreachable() -> std::sync::MutexGuard<'static, Option<Instant>> {
    UNREACHABLE_AT.lock().unwrap_or_else(|err| err.into_inner())
}

pub(crate) fn unreachable_recently() -> bool {
    lock_unreachable().is_some_and(|at| at.elapsed() < UNREACHABLE_RETRY)
}

// The metadata server is the last credentials source. When it cannot be reached, the process has no credentials.
pub(crate) async fn metadata_project(
    http: &reqwest::Client,
    url: &str,
) -> Result<Option<String>, Exception> {
    let Ok(response) = metadata_get(http, url).await else {
        return Ok(None);
    };
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(exception!(
            code = "GCP_AUTH",
            format!("metadata server project status {status}"),
        ));
    }
    let body = read_limited(response, "GCP_TRANSPORT", "metadata server project").await?;
    checked_project(String::from_utf8_lossy(&body).trim()).map(Some)
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

pub(crate) fn decode_access_response(
    status: u16,
    body: &[u8],
) -> Result<Option<String>, Exception> {
    if status == 404 {
        return Ok(None);
    }
    if !(200..300).contains(&status) {
        return Err(status_error(status, body));
    }
    decode_payload(body).map(Some)
}

pub(crate) fn secret_manager_http_client() -> Result<reqwest::Client, Exception> {
    http_client_builder().build().map_err(|err| {
        exception!(
            code = "GCP_TRANSPORT",
            "cannot build the HTTP client",
            source = err,
        )
    })
}

#[derive(serde::Deserialize)]
struct AccessResponse {
    payload: Payload,
}

#[derive(serde::Deserialize)]
struct Payload {
    data: String,
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

fn status_error(status: u16, body: &[u8]) -> Exception {
    let detail = google_error_message(&String::from_utf8_lossy(body)).unwrap_or_default();
    let kind = match status {
        401 | 403 => "GCP_AUTH",
        _ => "GCP_API",
    };
    exception!(
        code = kind,
        format!("secret manager status {status}: {}", truncate(&detail, 300)),
    )
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

fn is_name_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}
