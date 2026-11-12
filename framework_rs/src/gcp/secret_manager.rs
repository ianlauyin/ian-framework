use google_cloud_secretmanager_v1::client::SecretManagerService;
use tokio::sync::OnceCell;

use crate::exception;
use crate::exception::Exception;
use crate::gcp::error::{error_code, to_exception};
use crate::gcp::project::{is_safe_name, resolve_project};

// None means that the process has no project.
static SECRET_MANAGER: OnceCell<Option<SecretManager>> = OnceCell::const_new();

pub struct SecretManager {
    client: SecretManagerService,
    project: String,
}

impl SecretManager {
    pub async fn shared() -> Result<Option<&'static SecretManager>, Exception> {
        let state = SECRET_MANAGER.get_or_try_init(Self::connect).await?;
        Ok(state.as_ref())
    }

    // None means that the secret or its latest version does not exist.
    pub async fn access_latest(&self, secret: &str) -> Result<Option<String>, Exception> {
        if !is_safe_name(secret) {
            return Err(exception!(
                code = "GCP_CONFIG",
                "secret id has a character that is not valid",
            ));
        }
        let name = format!("projects/{}/secrets/{secret}/versions/latest", self.project);
        let response = match self
            .client
            .access_secret_version()
            .set_name(name)
            .send()
            .await
        {
            Ok(response) => response,
            Err(err) if error_code(&err) == "GCS_NOT_FOUND" => return Ok(None),
            Err(err) => return Err(to_exception("secret manager access failed", err)),
        };
        let data = response.payload.map(|payload| payload.data);
        parse_secret_value(data.as_deref().unwrap_or_default()).map(Some)
    }

    async fn connect() -> Result<Option<Self>, Exception> {
        let Some(project) = resolve_project().await? else {
            return Ok(None);
        };
        let client = SecretManagerService::builder()
            .build()
            .await
            .map_err(|err| {
                exception!(code = "GCP_AUTH", "cannot build the client", source = err)
            })?;
        Ok(Some(Self { client, project }))
    }
}

// The value is a secret, so no error holds the value or a parser error that can quote it.
pub(crate) fn parse_secret_value(data: &[u8]) -> Result<String, Exception> {
    let value = std::str::from_utf8(data)
        .map_err(|_| exception!(code = "GCP_CONFIG", "secret value is not UTF-8"))?
        .trim();
    if value.is_empty() {
        return Err(exception!(code = "GCP_CONFIG", "secret value is empty"));
    }
    Ok(value.to_string())
}
