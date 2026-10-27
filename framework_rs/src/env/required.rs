use std::env::{self, VarError};

use crate::env::{EnvError, EnvErrorKind};

pub fn required(name: &str) -> Result<String, EnvError> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(value.trim().to_string()),
        Ok(_) | Err(VarError::NotPresent) => Err(EnvError::new(EnvErrorKind::Missing, name)),
        Err(VarError::NotUnicode(_)) => Err(EnvError::new(EnvErrorKind::NotUnicode, name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::tests::{not_unicode, with_var};
    use std::ffi::OsStr;

    const NAME: &str = "IAN_FRAMEWORK_TEST_REQUIRED";

    fn read(value: Option<&str>) -> Result<String, EnvError> {
        with_var(NAME, value.map(OsStr::new), || required(NAME))
    }

    #[test]
    fn gives_the_trimmed_value() {
        assert_eq!(read(Some("value")), Ok("value".to_string()));
        assert_eq!(read(Some("  value \n")), Ok("value".to_string()));
    }

    #[test]
    fn rejects_an_absent_or_blank_var() {
        let missing = EnvError::new(EnvErrorKind::Missing, NAME);
        assert_eq!(read(None), Err(missing.clone()));
        assert_eq!(read(Some("")), Err(missing.clone()));
        assert_eq!(read(Some(" \t")), Err(missing));
    }

    #[test]
    fn rejects_a_value_that_is_not_unicode() {
        let error = with_var(NAME, Some(not_unicode()), || required(NAME)).unwrap_err();
        assert_eq!(error.kind(), EnvErrorKind::NotUnicode);
        assert_eq!(error.name(), NAME);
    }
}
