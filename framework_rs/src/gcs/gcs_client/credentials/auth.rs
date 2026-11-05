use crate::exception;
use crate::exception::Exception;
use crate::gcp::{
    metadata_get, metadata_token, truncate, CachedToken, Credentials, UserCredentials,
};
use crate::gcs::from_gcp;

const METADATA_EMAIL_URL: &str =
    "http://169.254.169.254/computeMetadata/v1/instance/service-accounts/default/email";

#[derive(Clone)]
pub(crate) enum Auth {
    Metadata,

    User {
        credentials: UserCredentials,
        signer_email: String,
    },
}

impl Auth {
    pub(crate) async fn from_adc(signer_email: Option<&str>) -> Result<Self, Exception> {
        let credentials = Credentials::from_adc().await.map_err(from_gcp)?;
        auth_from_credentials(credentials, signer_email)
    }

    pub(crate) fn validate(&self) -> Result<(), Exception> {
        let Auth::User {
            credentials,
            signer_email,
        } = self
        else {
            return Ok(());
        };
        credentials.validate().map_err(from_gcp)?;
        validate_signer_email(signer_email)
    }

    pub(crate) async fn signer_email(&self, http: &reqwest::Client) -> Result<String, Exception> {
        match self {
            Auth::Metadata => metadata_email(http, METADATA_EMAIL_URL).await,
            Auth::User { signer_email, .. } => Ok(signer_email.clone()),
        }
    }

    pub(crate) async fn fetch_token(
        &self,
        http: &reqwest::Client,
        metadata_token_url: &str,
    ) -> Result<CachedToken, Exception> {
        match self {
            Auth::Metadata => metadata_token(http, metadata_token_url)
                .await
                .map_err(from_gcp),
            Auth::User { credentials, .. } => credentials.fetch_token(http).await.map_err(from_gcp),
        }
    }
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

pub(crate) async fn metadata_email(http: &reqwest::Client, url: &str) -> Result<String, Exception> {
    let response = metadata_get(http, url).await.map_err(from_gcp)?;
    let status = response.status();
    let body = response.text().await.map_err(|err| {
        exception!(
            code = "GCS_AUTH",
            "metadata server email response is not text",
            source = err.without_url(),
        )
    })?;
    if !status.is_success() {
        return Err(exception!(
            code = "GCS_AUTH",
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

fn validate_signer_email(email: &str) -> Result<(), Exception> {
    if !email.contains('@') || email.contains(char::is_whitespace) {
        return Err(exception!(
            code = "GCS_CONFIG",
            "signer email is not a service account email",
        ));
    }
    Ok(())
}

pub(crate) fn auth_from_credentials(
    credentials: Credentials,
    signer_email: Option<&str>,
) -> Result<Auth, Exception> {
    match credentials {
        Credentials::Metadata => Ok(Auth::Metadata),
        Credentials::User(credentials) => {
            let signer_email = signer_email.ok_or_else(|| {
                exception!(
                    code = "GCS_CONFIG",
                    "user application default credentials require a signer email",
                )
            })?;
            // GcsClient::new checks that the fields are not empty and the signer email is valid.
            Ok(Auth::User {
                credentials,
                signer_email: signer_email.to_string(),
            })
        }
    }
}
