use std::sync::Arc;
use std::time::Duration;

use super::super::cred::{validate_auth, Auth};
use super::super::token;
#[cfg(target_os = "linux")]
use super::super::TRANSFER_TIMEOUT;
use super::super::{GcsBackend, GcsError, GcsErrorKind};
use super::Inner;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

impl GcsBackend {
    pub(crate) async fn new(bucket_name: impl Into<String>, auth: Auth) -> Result<Self, GcsError> {
        let bucket_name = bucket_name.into();
        validate_bucket(&bucket_name)?;
        validate_auth(&auth)?;
        let http = http_client()?;
        let email = match &auth {
            Auth::Metadata => token::metadata_email(&http).await?,
            Auth::User { signer_email, .. } => signer_email.clone(),
        };
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
    use super::super::super::cred::Auth;
    use super::super::super::{GcsBackend, GcsErrorKind};

    fn user_auth() -> Auth {
        Auth::User {
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            refresh_token: "refresh".to_string(),
            signer_email: "signer@example.iam.gserviceaccount.com".to_string(),
        }
    }

    #[tokio::test]
    async fn user_backend_keeps_bucket_and_signer() {
        let backend = GcsBackend::new("my-bucket", user_auth()).await.unwrap();
        assert_eq!(backend.bucket(), "my-bucket");
        assert_eq!(
            backend.signer_email(),
            "signer@example.iam.gserviceaccount.com"
        );
        let cloned = backend.clone();
        assert_eq!(cloned.bucket(), "my-bucket");
    }

    #[tokio::test]
    async fn backend_rejects_an_empty_bucket() {
        let err = GcsBackend::new(" ", user_auth()).await.unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Config);
    }

    #[tokio::test]
    async fn backend_rejects_an_incomplete_user() {
        let auth = Auth::User {
            client_id: String::new(),
            client_secret: "secret".to_string(),
            refresh_token: "refresh".to_string(),
            signer_email: "signer@example.iam.gserviceaccount.com".to_string(),
        };
        let err = GcsBackend::new("my-bucket", auth).await.unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Config);
    }
}
