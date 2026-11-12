use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::sync::{Mutex, MutexGuard};

use crate::env::*;
use crate::exception::Exception;

const OPTIONAL_NAME: &str = "IAN_FRAMEWORK_TEST_OPTIONAL";
const REQUIRED_NAME: &str = "IAN_FRAMEWORK_TEST_REQUIRED";

pub(crate) fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

pub(crate) fn not_unicode() -> &'static OsStr {
    OsStr::from_bytes(&[0x66, 0x6f, 0x80])
}

pub(crate) fn with_var<T>(name: &str, value: Option<&OsStr>, read: impl FnOnce() -> T) -> T {
    with_vars(&[(name, value)], read)
}

pub(crate) fn with_vars<T>(vars: &[(&str, Option<&OsStr>)], read: impl FnOnce() -> T) -> T {
    let _guard = env_lock();
    let previous: Vec<_> = vars
        .iter()
        .map(|(name, _)| (*name, std::env::var_os(name)))
        .collect();
    for (name, value) in vars {
        set_or_remove(name, *value);
    }
    let result = read();
    for (name, value) in &previous {
        set_or_remove(name, value.as_deref());
    }
    result
}

#[cfg(feature = "gcp")]
pub(crate) fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

fn set_or_remove(name: &str, value: Option<&OsStr>) {
    match value {
        Some(value) => std::env::set_var(name, value),
        None => std::env::remove_var(name),
    }
}

fn read_optional(value: Option<&str>) -> Result<Option<String>, Exception> {
    with_var(OPTIONAL_NAME, value.map(OsStr::new), || {
        optional(OPTIONAL_NAME)
    })
}

fn read_required(value: Option<&str>) -> Result<String, Exception> {
    with_var(REQUIRED_NAME, value.map(OsStr::new), || {
        required(REQUIRED_NAME)
    })
}

#[test]
fn optional_gives_the_trimmed_env_var() {
    assert_eq!(
        read_optional(Some(" value \n")).unwrap().as_deref(),
        Some("value")
    );
}

#[test]
fn optional_gives_none_for_an_absent_or_blank_var() {
    assert_eq!(read_optional(None).unwrap(), None);
    assert_eq!(read_optional(Some("")).unwrap(), None);
    assert_eq!(read_optional(Some(" \t")).unwrap(), None);
}

#[test]
fn optional_rejects_a_value_that_is_not_unicode() {
    let error = with_var(OPTIONAL_NAME, Some(not_unicode()), || {
        optional(OPTIONAL_NAME)
    })
    .unwrap_err();
    assert_eq!(error.code(), "ENV_NOT_UNICODE");
    assert_eq!(
        error.message(),
        format!("{OPTIONAL_NAME} is not valid unicode")
    );
}

#[test]
fn required_gives_the_trimmed_value() {
    assert_eq!(read_required(Some("value")).unwrap(), "value");
    assert_eq!(read_required(Some("  value \n")).unwrap(), "value");
}

#[test]
fn required_rejects_an_absent_or_blank_var() {
    for value in [None, Some(""), Some(" \t")] {
        let error = read_required(value).unwrap_err();
        assert_eq!(error.code(), "ENV_MISSING");
        assert_eq!(error.message(), format!("{REQUIRED_NAME} is required"));
    }
}

#[test]
fn required_rejects_a_value_that_is_not_unicode() {
    let error = with_var(REQUIRED_NAME, Some(not_unicode()), || {
        required(REQUIRED_NAME)
    })
    .unwrap_err();
    assert_eq!(error.code(), "ENV_NOT_UNICODE");
    assert!(error.message().contains(REQUIRED_NAME));
}
