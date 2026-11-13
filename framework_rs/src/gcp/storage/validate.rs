use std::collections::BTreeMap;

use crate::exception;
use crate::exception::Exception;

pub const BUCKET_NAME_INVALID: &str = "BUCKET_NAME_INVALID";
pub const METADATA_INVALID: &str = "METADATA_INVALID";
pub const OBJECT_NAME_INVALID: &str = "OBJECT_NAME_INVALID";

// GCS refuses a longer object name.
pub(crate) const MAX_OBJECT_NAME_BYTES: usize = 1024;

pub(crate) fn validate_object_name(name: &str) -> Result<(), Exception> {
    if name.is_empty()
        || name.len() > MAX_OBJECT_NAME_BYTES
        || name == "."
        || name == ".."
        || name.contains(['\r', '\n'])
    {
        return Err(exception!(
            code = OBJECT_NAME_INVALID,
            "object name is empty, too long, `.`, `..` or contains a line break",
        ));
    }
    Ok(())
}

pub(crate) fn validate_metadata(metadata: &BTreeMap<String, String>) -> Result<(), Exception> {
    for (key, value) in metadata {
        if key.is_empty() || key.contains(['\r', '\n']) || value.contains(['\r', '\n']) {
            return Err(exception!(
                code = METADATA_INVALID,
                "metadata key is empty or metadata contains a line break",
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_bucket(bucket_name: &str) -> Result<(), Exception> {
    if bucket_name.is_empty()
        || bucket_name.contains(['/', '\\', ' ', '\r', '\n'])
        || !bucket_name.is_ascii()
    {
        return Err(exception!(
            code = BUCKET_NAME_INVALID,
            "bucket name is empty or contains a forbidden character",
        ));
    }
    Ok(())
}
