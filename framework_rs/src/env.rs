use std::env;

use crate::exception;
use crate::exception::Exception;

pub const ENV_MISSING: &str = "ENV_MISSING";
pub const ENV_NOT_UNICODE: &str = "ENV_NOT_UNICODE";

pub fn optional(name: &str) -> Result<Option<String>, Exception> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value.trim().to_string())),
        Ok(_) | Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(exception!(
            code = ENV_NOT_UNICODE,
            format!("{name} is not valid unicode"),
        )),
    }
}

// The exception never holds the value of the variable, because the value can be a secret.
pub fn required(name: &str) -> Result<String, Exception> {
    optional(name)?.ok_or_else(|| exception!(code = ENV_MISSING, format!("{name} is required")))
}
