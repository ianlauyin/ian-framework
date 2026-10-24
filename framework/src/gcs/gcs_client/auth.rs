use std::time::{Duration, Instant};

use crate::gcs::encode::encode_seg;
use crate::gcs::gcs_client::{api_detail, truncate};
use crate::gcs::{GcsError, GcsErrorKind};

const METADATA_ROOT: &str =
    "http://169.254.169.254/computeMetadata/v1/instance/service-accounts/default";
const CLOUD_PLATFORM_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const METADATA_TIMEOUT: Duration = Duration::from_secs(2);
const TOKEN_HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const TOKEN_SKEW: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub(crate) enum Auth {
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
    pub(crate) fn validate(&self) -> Result<(), GcsError> {
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

    pub(crate) async fn signer_email(&self, http: &reqwest::Client) -> Result<String, GcsError> {
        match self {
            Auth::Metadata => metadata_email(http).await,
            Auth::User { signer_email, .. } => Ok(signer_email.clone()),
        }
    }

    pub(crate) async fn fetch_token(
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

pub(crate) struct CachedToken {
    pub(crate) value: String,
    pub(crate) expires_at: Instant,
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
