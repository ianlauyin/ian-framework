use std::collections::BTreeMap;

use crate::gcs::{GcsError, GcsErrorKind};

const DEFAULT_CONTENT_TYPE: &str = "application/octet-stream";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PutOptions {
    pub content_type: Option<String>,
    pub metadata: BTreeMap<String, String>,
}

impl PutOptions {
    pub(super) fn content_type(&self) -> Result<String, GcsError> {
        match &self.content_type {
            Some(value) => {
                if value.is_empty() || !value.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
                    return Err(GcsError::new(
                        GcsErrorKind::Config,
                        "content type is empty or is not printable ASCII",
                    ));
                }
                Ok(value.clone())
            }
            None => Ok(DEFAULT_CONTENT_TYPE.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_options_do_not_insert_product_keys() {
        let options = PutOptions::default();
        assert!(options.metadata.is_empty());
        assert_eq!(options.content_type().unwrap(), "application/octet-stream");
    }
}
