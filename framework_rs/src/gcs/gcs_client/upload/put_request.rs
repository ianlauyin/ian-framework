use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::exception;
use crate::exception::Exception;

const DEFAULT_CONTENT_TYPE: &str = "application/octet-stream";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PutRequest {
    pub name: String,
    pub body: PutBody,
    pub options: PutOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PutBody {
    Bytes(Vec<u8>),
    File(PathBuf),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PutOptions {
    pub content_type: Option<String>,
    pub metadata: BTreeMap<String, String>,
}

impl PutOptions {
    pub(crate) fn content_type(&self) -> Result<String, Exception> {
        match &self.content_type {
            Some(value) => {
                if value.is_empty() || !value.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
                    return Err(exception!(
                        "content type is empty or is not printable ASCII",
                        code = "GCS_CONFIG"
                    ));
                }
                Ok(value.clone())
            }
            None => Ok(DEFAULT_CONTENT_TYPE.to_string()),
        }
    }
}
