use std::env;

use crate::env::not_unicode;
use crate::exception::Exception;
#[cfg(feature = "gcp")]
use crate::gcp::SecretManager;

pub async fn optional(name: &str) -> Result<Option<String>, Exception> {
    #[cfg(feature = "gcp")]
    {
        let manager = SecretManager::shared()
            .await
            .map_err(|err| crate::env::secret(name, err))?;
        if let Some(value) = secret(name, manager).await? {
            return Ok(Some(value));
        }
    }
    read_var(name)
}

#[cfg(feature = "gcp")]
pub(crate) async fn secret(
    name: &str,
    manager: Option<&SecretManager>,
) -> Result<Option<String>, Exception> {
    let Some(manager) = manager else {
        return Ok(None);
    };
    manager
        .access_latest(name)
        .await
        .map_err(|err| crate::env::secret(name, err))
}

fn read_var(name: &str) -> Result<Option<String>, Exception> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value.trim().to_string())),
        Ok(_) | Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(not_unicode(name)),
    }
}
