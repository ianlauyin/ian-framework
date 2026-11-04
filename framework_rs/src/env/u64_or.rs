use std::env;

use crate::env::{invalid, not_unicode};
use crate::exception::Exception;

pub fn u64_or(name: &str, default: u64) -> Result<u64, Exception> {
    match env::var(name) {
        Ok(value) => value.parse().map_err(|_| invalid(name)),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(env::VarError::NotUnicode(_)) => Err(not_unicode(name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::tests::{not_unicode as not_unicode_value, with_var};
    use std::ffi::OsStr;

    const NAME: &str = "IAN_FRAMEWORK_TEST_U64_OR";

    fn read(value: Option<&str>) -> Result<u64, Exception> {
        with_var(NAME, value.map(OsStr::new), || u64_or(NAME, 7))
    }

    #[test]
    fn gives_the_default_when_the_var_is_absent() {
        assert_eq!(read(None).unwrap(), 7);
    }

    #[test]
    fn parses_the_value() {
        assert_eq!(read(Some("0")).unwrap(), 0);
        assert_eq!(read(Some("42")).unwrap(), 42);
        assert_eq!(read(Some("18446744073709551615")).unwrap(), u64::MAX);
    }

    #[test]
    fn rejects_a_value_that_is_not_a_u64() {
        for value in ["", " 42", "-1", "1.5", "abc", "18446744073709551616"] {
            let error = read(Some(value)).unwrap_err();
            assert_eq!(error.code(), invalid(NAME).code(), "{value:?}");
            assert_eq!(error.message(), invalid(NAME).message(), "{value:?}");
        }
    }

    #[test]
    fn the_error_does_not_show_the_value() {
        let error = read(Some("secret-value")).unwrap_err();
        assert!(!format!("{error} {error:?}").contains("secret-value"));
    }

    #[test]
    fn rejects_a_value_that_is_not_unicode() {
        let error = with_var(NAME, Some(not_unicode_value()), || u64_or(NAME, 7)).unwrap_err();
        assert_eq!(error.code(), "ENV_NOT_UNICODE");
    }
}
