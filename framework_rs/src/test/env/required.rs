use crate::env::missing;
use crate::exception::Exception;

use std::ffi::OsStr;

use crate::env::required::*;
use crate::test::env::{block_on, not_unicode, with_var};

const NAME: &str = "IAN_FRAMEWORK_TEST_REQUIRED";

fn read(value: Option<&str>) -> Result<String, Exception> {
    with_var(NAME, value.map(OsStr::new), || block_on(required(NAME)))
}

#[test]
fn gives_the_trimmed_value() {
    assert_eq!(read(Some("value")).unwrap(), "value");
    assert_eq!(read(Some("  value \n")).unwrap(), "value");
}

#[test]
fn rejects_an_absent_or_blank_var() {
    for value in [None, Some(""), Some(" \t")] {
        let error = read(value).unwrap_err();
        assert_eq!(error.code(), "ENV_MISSING");
        assert_eq!(error.message(), missing(NAME).message());
    }
}

#[test]
fn rejects_a_value_that_is_not_unicode() {
    let error = with_var(NAME, Some(not_unicode()), || block_on(required(NAME))).unwrap_err();
    assert_eq!(error.code(), "ENV_NOT_UNICODE");
    assert!(error.message().contains(NAME));
}

#[test]
fn the_future_is_send() {
    fn assert_send<T: Send>(_: &T) {}
    let future = required(NAME);
    assert_send(&future);
}
