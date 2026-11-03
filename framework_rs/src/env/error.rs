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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_gives_one_message_for_each_code() {
        assert_eq!(missing("APP_KEY").to_string(), "APP_KEY is required");
        assert_eq!(
            not_unicode("APP_KEY").to_string(),
            "APP_KEY is not valid unicode"
        );
        assert_eq!(invalid("APP_KEY").to_string(), "APP_KEY is not valid");
    }

    #[test]
    fn missing_and_invalid_keep_their_code() {
        assert_eq!(missing("APP_KEY").code(), Some("ENV_MISSING"));
        assert_eq!(invalid("APP_KEY").code(), Some("ENV_INVALID"));
    }

    #[cfg(feature = "gcp")]
    #[test]
    fn secret_keeps_the_source_in_the_trace() {
        let error = secret("APP_KEY", exception!("denied", code = "GCP_AUTH"));
        assert_eq!(error.code(), Some("ENV_SECRET"));
        assert_eq!(
            error.to_string(),
            "APP_KEY cannot be read from Secret Manager"
        );
        let cause = error.source_exception().unwrap();
        assert_eq!(cause.to_string(), "denied");
    }
}
