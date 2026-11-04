use crate::env::not_unicode;
use crate::exception::Exception;

use std::ffi::OsStr;

use crate::env::optional::*;
use crate::test::env::{block_on, not_unicode as not_unicode_value, with_var};
#[cfg(feature = "gcp")]
#[cfg(feature = "gcp")]
use crate::test::gcp::{fake_secret_manager, ok_body, serve};

const NAME: &str = "IAN_FRAMEWORK_TEST_OPTIONAL";

fn read(value: Option<&str>) -> Result<Option<String>, Exception> {
    with_var(NAME, value.map(OsStr::new), || block_on(optional(NAME)))
}

#[test]
fn gives_the_trimmed_env_var() {
    assert_eq!(read(Some(" value \n")).unwrap().as_deref(), Some("value"));
}

#[test]
fn an_absent_or_blank_var_is_none() {
    assert_eq!(read(None).unwrap(), None);
    assert_eq!(read(Some("")).unwrap(), None);
    assert_eq!(read(Some(" \t")).unwrap(), None);
}

#[test]
fn rejects_a_value_that_is_not_unicode() {
    let error = with_var(NAME, Some(not_unicode_value()), || block_on(optional(NAME))).unwrap_err();
    assert_eq!(error.code(), not_unicode(NAME).code());
    assert_eq!(error.message(), not_unicode(NAME).message());
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
    assert_eq!(value.unwrap().as_deref(), Some("from-secret"));
}

#[cfg(feature = "gcp")]
#[test]
fn a_404_gives_no_secret_so_the_env_var_is_read() {
    let body = br#"{"error":{"code":404,"message":"not found"}}"#;
    assert_eq!(secret_with(404, body).unwrap(), None);
}

#[cfg(feature = "gcp")]
#[test]
fn no_credentials_gives_no_secret_so_the_env_var_is_read() {
    assert_eq!(block_on(secret(SECRET_NAME, None)).unwrap(), None);
    // block_on sets the shared Secret Manager state to "no credentials".
    assert_eq!(
        read(Some(" from-env ")).unwrap().as_deref(),
        Some("from-env")
    );
}

#[cfg(feature = "gcp")]
#[test]
fn a_403_is_an_error_without_fallback() {
    let body = br#"{"error":{"code":403,"message":"Permission denied"}}"#;
    let error = secret_with(403, body).unwrap_err();
    assert_eq!(error.code(), "ENV_SECRET");
    assert_eq!(
        error.to_string(),
        "IAN_FRAMEWORK_TEST_SECRET cannot be read from Secret Manager"
    );
    assert_eq!(
        error.source_exception().unwrap().to_string(),
        "secret manager status 403: Permission denied"
    );
}

#[cfg(feature = "gcp")]
#[test]
fn a_bad_secret_value_is_an_error_without_fallback() {
    for body in [ok_body(b" "), ok_body(&[0x66, 0x80]), b"{}".to_vec()] {
        let error = secret_with(200, &body).unwrap_err();
        assert_eq!(error.code(), "ENV_SECRET");
    }
}

#[cfg(not(feature = "gcp"))]
#[test]
fn without_gcp_only_the_env_var_is_read() {
    assert_eq!(read(Some("from-env")).unwrap().as_deref(), Some("from-env"));
}
