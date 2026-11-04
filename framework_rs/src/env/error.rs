use crate::exception;
use crate::exception::Exception;

// The exception never holds the value of the variable, because the value can be a secret.
pub fn missing(name: &str) -> Exception {
    exception!(format!("{name} is required"), code = "ENV_MISSING")
}

pub fn invalid(name: &str) -> Exception {
    exception!(format!("{name} is not valid"), code = "ENV_INVALID")
}

pub(crate) fn not_unicode(name: &str) -> Exception {
    exception!(
        format!("{name} is not valid unicode"),
        code = "ENV_NOT_UNICODE"
    )
}

#[cfg(feature = "gcp")]
pub(crate) fn secret(name: &str, source: Exception) -> Exception {
    exception!(
        format!("{name} cannot be read from Secret Manager"),
        code = "ENV_SECRET",
        source = source
    )
}
