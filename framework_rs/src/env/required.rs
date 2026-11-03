use crate::env::{missing, optional};
use crate::exception::Exception;

pub async fn required(name: &str) -> Result<String, Exception> {
    optional(name).await?.ok_or_else(|| missing(name))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;
    use crate::env::tests::{block_on, not_unicode, with_var};

    const NAME: &str = "IAN_FRAMEWORK_TEST_REQUIRED";

    fn read(value: Option<&str>) -> Result<String, Exception> {
        with_var(NAME, value.map(OsStr::new), || block_on(required(NAME)))
    }

    #[test]
    fn gives_the_trimmed_value() {
        assert_eq!(read(Some("value")), Ok("value".to_string()));
        assert_eq!(read(Some("  value \n")), Ok("value".to_string()));
    }

    #[test]
    fn rejects_an_absent_or_blank_var() {
        let missing = missing(NAME);
        assert_eq!(read(None), Err(missing.clone()));
        assert_eq!(read(Some("")), Err(missing.clone()));
        assert_eq!(read(Some(" \t")), Err(missing));
    }

    #[test]
    fn rejects_a_value_that_is_not_unicode() {
        let error = with_var(NAME, Some(not_unicode()), || block_on(required(NAME))).unwrap_err();
        assert_eq!(error.code(), Some("ENV_NOT_UNICODE"));
        assert!(error.message().contains(NAME));
    }

    #[test]
    fn the_future_is_send() {
        fn assert_send<T: Send>(_: &T) {}
        let future = required(NAME);
        assert_send(&future);
    }
}
