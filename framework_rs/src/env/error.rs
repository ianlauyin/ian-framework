use crate::exception;
use crate::exception::code;
use crate::exception::Exception;

// The exception never holds the value of the variable, because the value can be a secret.
pub fn missing(name: &str) -> Exception {
    exception!(code::ENV_MISSING, format!("{name} is required"))
}

pub fn invalid(name: &str) -> Exception {
    exception!(code::ENV_INVALID, format!("{name} is not valid"))
}

pub(crate) fn not_unicode(name: &str) -> Exception {
    exception!(
        code::ENV_NOT_UNICODE,
        format!("{name} is not valid unicode")
    )
}

#[cfg(feature = "gcp")]
pub(crate) fn secret(name: &str, source: Exception) -> Exception {
    exception!(
        code::ENV_SECRET,
        format!("{name} cannot be read from Secret Manager"),
        caused_by = source
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_gives_one_message_for_each_code() {
        assert_eq!(
            missing("APP_KEY").to_string(),
            "env_missing: APP_KEY is required"
        );
        assert_eq!(
            not_unicode("APP_KEY").to_string(),
            "env_not_unicode: APP_KEY is not valid unicode"
        );
        assert_eq!(
            invalid("APP_KEY").to_string(),
            "env_invalid: APP_KEY is not valid"
        );
    }

    #[test]
    fn missing_and_invalid_keep_their_code() {
        assert_eq!(missing("APP_KEY").code(), code::ENV_MISSING);
        assert_eq!(invalid("APP_KEY").code(), code::ENV_INVALID);
    }

    #[cfg(feature = "gcp")]
    #[test]
    fn secret_keeps_the_source_in_the_trace() {
        let error = secret("APP_KEY", exception!(code::GCP_AUTH, "denied"));
        assert_eq!(error.code(), code::ENV_SECRET);
        assert_eq!(
            error.to_string(),
            "env_secret: APP_KEY cannot be read from Secret Manager"
        );
        let cause = error.source_exception().unwrap();
        assert_eq!(cause.to_string(), "gcp_auth: denied");
    }
}
