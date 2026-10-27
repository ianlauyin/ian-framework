use std::env;

use crate::gcp::{
    metadata_get, metadata_token, truncate, CachedToken, Credentials, UserCredentials,
};
use crate::gcs::{GcsError, GcsErrorKind};

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
    pub(crate) async fn from_adc() -> Result<Self, GcsError> {
        let credentials = Credentials::from_adc().await?;
        auth_from_credentials(credentials, signer_email_from_env().as_deref())
    }

    pub(crate) fn validate(&self) -> Result<(), GcsError> {
        let Auth::User {
            credentials,
            signer_email,
        } = self
        else {
            return Ok(());
        };
        credentials.validate()?;
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
        let token = match self {
            Auth::Metadata => metadata_token(http).await?,
            Auth::User { credentials, .. } => credentials.fetch_token(http).await?,
        };
        Ok(token)
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

async fn metadata_email(http: &reqwest::Client) -> Result<String, GcsError> {
    let response = metadata_get(http, METADATA_EMAIL_URL).await?;
    let status = response.status();
    let body = response.text().await.map_err(|err| {
        GcsError::with_source(
            GcsErrorKind::Auth,
            "metadata server email response is not text",
            err.without_url(),
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

fn validate_signer_email(email: &str) -> Result<(), GcsError> {
    if !email.contains('@') || email.contains(char::is_whitespace) {
        return Err(GcsError::new(
            GcsErrorKind::Config,
            "signer email is not a service account email",
        ));
    }
    Ok(())
}

fn signer_email_from_env() -> Option<String> {
    let value = env::var("GCS_SIGNER_EMAIL").ok()?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_string())
}

fn auth_from_credentials(
    credentials: Credentials,
    signer_email: Option<&str>,
) -> Result<Auth, GcsError> {
    match credentials {
        Credentials::Metadata => Ok(Auth::Metadata),
        Credentials::User(credentials) => {
            let signer_email = signer_email.ok_or_else(|| {
                GcsError::new(
                    GcsErrorKind::Config,
                    "user application default credentials require GCS_SIGNER_EMAIL",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn user_credentials() -> Credentials {
        Credentials::User(UserCredentials {
            client_id: "id.apps.googleusercontent.com".to_string(),
            client_secret: "secret".to_string(),
            refresh_token: "refresh".to_string(),
            quota_project_id: None,
        })
    }

    #[test]
    fn metadata_credentials_need_no_signer_email() {
        let auth = auth_from_credentials(Credentials::Metadata, None).unwrap();
        assert!(matches!(auth, Auth::Metadata));
    }

    #[test]
    fn user_adc_requires_a_signer_email() {
        let err = auth_from_credentials(user_credentials(), None).unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Config);
        assert!(err.to_string().contains("GCS_SIGNER_EMAIL"));
    }

    #[test]
    fn user_adc_keeps_the_signer_email() {
        let auth = auth_from_credentials(
            user_credentials(),
            Some("signer@example.iam.gserviceaccount.com"),
        )
        .unwrap();
        match auth {
            Auth::User {
                credentials,
                signer_email,
            } => {
                assert_eq!(credentials.client_id, "id.apps.googleusercontent.com");
                assert_eq!(signer_email, "signer@example.iam.gserviceaccount.com");
            }
            Auth::Metadata => panic!("expected user auth"),
        }
    }

    #[test]
    fn debug_does_not_print_the_refresh_token() {
        let auth = auth_from_credentials(
            user_credentials(),
            Some("signer@example.iam.gserviceaccount.com"),
        )
        .unwrap();
        let text = format!("{auth:?}");
        assert!(text.contains("signer@example.iam.gserviceaccount.com"));
        assert!(!text.contains("refresh"));
        assert!(!text.contains("secret"));
    }
}
