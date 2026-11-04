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
