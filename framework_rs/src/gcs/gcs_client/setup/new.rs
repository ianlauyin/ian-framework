use std::sync::Arc;
use std::time::Duration;

#[cfg(target_os = "linux")]
use crate::gcs::gcs_client::TRANSFER_TIMEOUT;
use crate::gcs::gcs_client::{Auth, GcsClient, Inner};
use crate::gcs::{GcsError, GcsErrorKind};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

impl GcsClient {
    pub(crate) async fn new(bucket_name: impl Into<String>, auth: Auth) -> Result<Self, GcsError> {
        let bucket_name = bucket_name.into();
        validate_bucket(&bucket_name)?;
        auth.validate()?;
        let http = http_client()?;
        let email = auth.signer_email(&http).await?;
        Ok(Self {
            inner: Arc::new(Inner {
                bucket: bucket_name,
                email,
                auth,
                http,
                token: tokio::sync::Mutex::new(None),
                ops_log: std::sync::Mutex::new(None),
            }),
        })
    }
}

fn validate_bucket(bucket_name: &str) -> Result<(), GcsError> {
    if bucket_name.is_empty()
        || bucket_name.contains(['/', '\\', ' ', '\r', '\n'])
        || !bucket_name.is_ascii()
    {
        return Err(GcsError::new(
            GcsErrorKind::Config,
            "bucket name is empty or contains a forbidden character",
        ));
    }
    Ok(())
}

fn http_client() -> Result<reqwest::Client, GcsError> {
    let builder = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("ian_framework/", env!("CARGO_PKG_VERSION")));
    #[cfg(target_os = "linux")]
    let builder = builder.tcp_user_timeout(TRANSFER_TIMEOUT);
    builder.build().map_err(|err| {
        GcsError::with_source(GcsErrorKind::Transport, "cannot build the HTTP client", err)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_auth() -> Auth {
        Auth::User {
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            refresh_token: "refresh".to_string(),
            signer_email: "signer@example.iam.gserviceaccount.com".to_string(),
        }
    }

    #[tokio::test]
    async fn user_client_keeps_bucket_and_signer() {
        let client = GcsClient::new("my-bucket", user_auth()).await.unwrap();
        assert_eq!(client.bucket(), "my-bucket");
        assert_eq!(
            client.signer_email(),
            "signer@example.iam.gserviceaccount.com"
        );
        let cloned = client.clone();
        assert_eq!(cloned.bucket(), "my-bucket");
    }

    #[tokio::test]
    async fn client_rejects_an_empty_bucket() {
        let err = GcsClient::new(" ", user_auth()).await.unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Config);
    }

    #[tokio::test]
    async fn client_rejects_an_incomplete_user() {
        let auth = Auth::User {
            client_id: String::new(),
            client_secret: "secret".to_string(),
            refresh_token: "refresh".to_string(),
            signer_email: "signer@example.iam.gserviceaccount.com".to_string(),
        };
        let err = GcsClient::new("my-bucket", auth).await.unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Config);
    }
}
