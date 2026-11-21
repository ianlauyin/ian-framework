use std::sync::OnceLock;

use super::Logger;
use crate::exception;
use crate::exception::Exception;

static LOGGER: OnceLock<Logger> = OnceLock::new();

pub const LOGGER_ALREADY_SET: &str = "LOGGER_ALREADY_SET";

pub fn set(logger: Logger) -> Result<(), Exception> {
    if LOGGER.set(logger).is_err() {
        return Err(exception!(
            code = LOGGER_ALREADY_SET,
            "logger is already set"
        ));
    }
    Ok(())
}

pub fn get() -> Option<&'static Logger> {
    LOGGER.get()
}
