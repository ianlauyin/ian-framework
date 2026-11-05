#[cfg(feature = "gcp")]
use crate::exception;

use crate::env::error::*;

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
    assert_eq!(missing("APP_KEY").code(), "ENV_MISSING");
    assert_eq!(invalid("APP_KEY").code(), "ENV_INVALID");
}

#[cfg(feature = "gcp")]
#[test]
fn secret_keeps_the_source_in_the_trace() {
    let error = secret("APP_KEY", exception!(code = "GCP_AUTH", "denied"));
    assert_eq!(error.code(), "ENV_SECRET");
    assert_eq!(
        error.to_string(),
        "APP_KEY cannot be read from Secret Manager"
    );
    let cause = error.source_exception().unwrap();
    assert_eq!(cause.to_string(), "denied");
}
