use std::env;

use crate::exception;
use crate::exception::Exception;
#[cfg(feature = "gcp")]
use crate::gcp::SecretManager;

pub async fn optional(name: &str) -> Result<Option<String>, Exception> {
    #[cfg(feature = "gcp")]
    {
        let secret_error = |err: Exception| {
            exception!(
                code = "ENV_SECRET",
                format!("{name} cannot be read from Secret Manager"),
                source = err,
            )
        };
        let manager = SecretManager::shared().await.map_err(secret_error)?;
        let value = match manager {
            Some(manager) => manager.access_latest(name).await.map_err(secret_error)?,
            None => None,
        };
        if value.is_some() {
            return Ok(value);
        }
    }
    read_var(name)
}

// The exception never holds the value of the variable, because the value can be a secret.
pub async fn required(name: &str) -> Result<String, Exception> {
    optional(name)
        .await?
        .ok_or_else(|| exception!(code = "ENV_MISSING", format!("{name} is required")))
}

fn read_var(name: &str) -> Result<Option<String>, Exception> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value.trim().to_string())),
        Ok(_) | Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(exception!(
            code = "ENV_NOT_UNICODE",
            format!("{name} is not valid unicode"),
        )),
    }
}
