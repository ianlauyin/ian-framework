use std::time::{Duration, Instant};

use super::cred::Auth;
use super::{auth, http, token_cache, GcsBackend, GcsError, GcsErrorKind};

const METADATA_ROOT: &str =
    "http://169.254.169.254/computeMetadata/v1/instance/service-accounts/default";
const CLOUD_PLATFORM_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const METADATA_TIMEOUT: Duration = Duration::from_secs(2);
const TOKEN_HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const TOKEN_SKEW: Duration = Duration::from_secs(60);

pub(crate) struct CachedToken {
    value: String,
    expires_at: Instant,
}

pub(crate) async fn access_token(backend: &GcsBackend) -> Result<String, GcsError> {
    let mut guard = token_cache(backend).lock().await;
    if let Some(cached) = guard.as_ref() {
        if Instant::now() < cached.expires_at && !cached.value.is_empty() {
            return Ok(cached.value.clone());
        }
    }
    let message = match auth(backend) {
        Auth::Metadata => "request metadata server access token",
        Auth::User { .. } => "refresh user access token",
    };
    super::ops::record_op(
        backend,
        super::ops::OpsRecord {
            level: crate::logging::LogLevel::Debug,
            message,
            method: "",
            url: "",
            status: None,
            latency_ms: None,
            extra: &[],
        },
    );
    let fetched = fetch_token(http(backend), auth(backend)).await;
    if let Err(err) = &fetched {
        let error_text = super::ops::logged_error(err);
        super::ops::record_op(
            backend,
            super::ops::OpsRecord {
                level: crate::logging::LogLevel::Warn,
                message: "access token request failed",
                method: "",
                url: "",
                status: err.status(),
                latency_ms: None,
                extra: &[("error", &error_text)],
            },
        );
    }
    let (value, expires_at) = fetched?;
    *guard = Some(CachedToken {
        value: value.clone(),
        expires_at,
    });
    Ok(value)
}

pub(crate) async fn invalidate(backend: &GcsBackend) {
    *token_cache(backend).lock().await = None;
}

pub(crate) async fn metadata_email(http: &reqwest::Client) -> Result<String, GcsError> {
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
                super::truncate(&body, 300)
            ),
        ));
    }
    let email = body.trim().to_string();
    super::cred::validate_signer_email(&email)?;
    Ok(email)
}

async fn fetch_token(http: &reqwest::Client, auth: &Auth) -> Result<(String, Instant), GcsError> {
    match auth {
        Auth::Metadata => metadata_token(http).await,
        Auth::User {
            client_id,
            client_secret,
            refresh_token,
            ..
        } => user_token(http, client_id, client_secret, refresh_token).await,
    }
}

async fn metadata_token(http: &reqwest::Client) -> Result<(String, Instant), GcsError> {
    let url = format!(
        "{METADATA_ROOT}/token?scopes={}",
        super::encode::encode_seg(CLOUD_PLATFORM_SCOPE),
    );
    let response = metadata_get(http, &url).await?;
    token_from_response(response).await
}

async fn user_token(
    http: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<(String, Instant), GcsError> {
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

async fn token_from_response(response: reqwest::Response) -> Result<(String, Instant), GcsError> {
    let status = response.status();
    let body = response.text().await.map_err(|err| {
        GcsError::with_source(GcsErrorKind::Auth, "token response is not text", err)
    })?;
    if !status.is_success() {
        let detail = super::api_detail(&body).unwrap_or(body);
        return Err(GcsError::new(
            GcsErrorKind::Auth,
            format!(
                "token status {}: {}",
                status.as_u16(),
                super::truncate(&detail, 300)
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
    Ok((parsed.access_token, expiry(parsed.expires_in)))
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
