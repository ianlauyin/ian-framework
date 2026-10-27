use std::env::{self, VarError};

use crate::env::{EnvError, EnvErrorKind};

pub fn u64_or(name: &str, default: u64) -> Result<u64, EnvError> {
    match env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|_| EnvError::new(EnvErrorKind::Invalid, name)),
        Err(VarError::NotPresent) => Ok(default),
        Err(VarError::NotUnicode(_)) => Err(EnvError::new(EnvErrorKind::NotUnicode, name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::tests::{not_unicode, with_var};
    use std::ffi::OsStr;

    const NAME: &str = "IAN_FRAMEWORK_TEST_U64_OR";

    fn read(value: Option<&str>) -> Result<u64, EnvError> {
        with_var(NAME, value.map(OsStr::new), || u64_or(NAME, 7))
    }

    #[test]
    fn gives_the_default_when_the_var_is_absent() {
        assert_eq!(read(None), Ok(7));
    }

    #[test]
    fn parses_the_value() {
        assert_eq!(read(Some("0")), Ok(0));
        assert_eq!(read(Some("42")), Ok(42));
        assert_eq!(read(Some("18446744073709551615")), Ok(u64::MAX));
    }

    #[test]
    fn rejects_a_value_that_is_not_a_u64() {
        let invalid = EnvError::new(EnvErrorKind::Invalid, NAME);
        for value in ["", " 42", "-1", "1.5", "abc", "18446744073709551616"] {
            assert_eq!(read(Some(value)), Err(invalid.clone()), "{value:?}");
        }
    }

    #[test]
    fn the_error_does_not_show_the_value() {
        let error = read(Some("secret-value")).unwrap_err();
        assert!(!format!("{error} {error:?}").contains("secret-value"));
    }

    #[test]
    fn rejects_a_value_that_is_not_unicode() {
        let error = with_var(NAME, Some(not_unicode()), || u64_or(NAME, 7)).unwrap_err();
        assert_eq!(error.kind(), EnvErrorKind::NotUnicode);
    }
}
