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
async fn secret(name: &str, manager: Option<&SecretManager>) -> Result<Option<String>, Exception> {
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

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;
    use crate::env::tests::{block_on, not_unicode as not_unicode_value, with_var};
    #[cfg(feature = "gcp")]
    use crate::exception::code;
    #[cfg(feature = "gcp")]
    use crate::gcp::tests::{fake_secret_manager, ok_body, serve};

    const NAME: &str = "IAN_FRAMEWORK_TEST_OPTIONAL";

    fn read(value: Option<&str>) -> Result<Option<String>, Exception> {
        with_var(NAME, value.map(OsStr::new), || block_on(optional(NAME)))
    }

    #[test]
    fn gives_the_trimmed_env_var() {
        assert_eq!(read(Some(" value \n")), Ok(Some("value".to_string())));
    }

    #[test]
    fn an_absent_or_blank_var_is_none() {
        assert_eq!(read(None), Ok(None));
        assert_eq!(read(Some("")), Ok(None));
        assert_eq!(read(Some(" \t")), Ok(None));
    }

    #[test]
    fn rejects_a_value_that_is_not_unicode() {
        let error = with_var(NAME, Some(not_unicode_value()), || block_on(optional(NAME)));
        assert_eq!(error, Err(not_unicode(NAME)));
    }

    #[test]
    fn the_future_is_send() {
        fn assert_send<T: Send>(_: &T) {}
        let future = optional(NAME);
        assert_send(&future);
    }

    #[cfg(feature = "gcp")]
    const SECRET_NAME: &str = "IAN_FRAMEWORK_TEST_SECRET";

    #[cfg(feature = "gcp")]
    fn secret_with(status: u16, body: &[u8]) -> Result<Option<String>, Exception> {
        let body = body.to_vec();
        block_on(async move {
            let (addr, _server) = serve(status, body).await;
            secret(SECRET_NAME, Some(&fake_secret_manager(addr))).await
        })
    }

    #[cfg(feature = "gcp")]
    #[test]
    fn gives_the_secret() {
        let value = secret_with(200, &ok_body(b"from-secret"));
        assert_eq!(value, Ok(Some("from-secret".to_string())));
    }

    #[cfg(feature = "gcp")]
    #[test]
    fn a_404_gives_no_secret_so_the_env_var_is_read() {
        let body = br#"{"error":{"code":404,"message":"not found"}}"#;
        assert_eq!(secret_with(404, body), Ok(None));
    }

    #[cfg(feature = "gcp")]
    #[test]
    fn no_credentials_gives_no_secret_so_the_env_var_is_read() {
        assert_eq!(block_on(secret(SECRET_NAME, None)), Ok(None));
        // block_on sets the shared Secret Manager state to "no credentials".
        assert_eq!(read(Some(" from-env ")), Ok(Some("from-env".to_string())));
    }

    #[cfg(feature = "gcp")]
    #[test]
    fn a_403_is_an_error_without_fallback() {
        let body = br#"{"error":{"code":403,"message":"Permission denied"}}"#;
        let error = secret_with(403, body).unwrap_err();
        assert_eq!(error.code(), code::ENV_SECRET);
        assert_eq!(
            error.to_string(),
            "env_secret: IAN_FRAMEWORK_TEST_SECRET cannot be read from Secret Manager"
        );
        assert_eq!(
            error.source_exception().unwrap().to_string(),
            "gcp_auth: secret manager status 403: Permission denied"
        );
    }

    #[cfg(feature = "gcp")]
    #[test]
    fn a_bad_secret_value_is_an_error_without_fallback() {
        for body in [ok_body(b" "), ok_body(&[0x66, 0x80]), b"{}".to_vec()] {
            let error = secret_with(200, &body).unwrap_err();
            assert_eq!(error.code(), code::ENV_SECRET);
        }
    }

    #[cfg(not(feature = "gcp"))]
    #[test]
    fn without_gcp_only_the_env_var_is_read() {
        assert_eq!(read(Some("from-env")), Ok(Some("from-env".to_string())));
    }
}
