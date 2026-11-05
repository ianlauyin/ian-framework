use crate::exception;
use crate::exception::Exception;

// The exception never holds the value of the variable, because the value can be a secret.
pub fn missing(name: &str) -> Exception {
    exception!(code = "ENV_MISSING", format!("{name} is required"))
}

pub fn invalid(name: &str) -> Exception {
    exception!(code = "ENV_INVALID", format!("{name} is not valid"))
}

pub(crate) fn not_unicode(name: &str) -> Exception {
    exception!(
        code = "ENV_NOT_UNICODE",
        format!("{name} is not valid unicode"),
    )
}

#[cfg(feature = "gcp")]
pub(crate) fn secret(name: &str, source: Exception) -> Exception {
    exception!(
        code = "ENV_SECRET",
        format!("{name} cannot be read from Secret Manager"),
        source = source,
    )
}
