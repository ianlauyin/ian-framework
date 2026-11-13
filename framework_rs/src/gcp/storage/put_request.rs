use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::exception;
use crate::exception::Exception;

pub const CONTENT_TYPE_INVALID: &str = "CONTENT_TYPE_INVALID";

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
    pub(crate) fn content_type_or_default(&self) -> Result<&str, Exception> {
        match &self.content_type {
            Some(value) => {
                if value.is_empty() || !value.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
                    return Err(exception!(
                        code = CONTENT_TYPE_INVALID,
                        "content type is empty or is not printable ASCII",
                    ));
                }
                Ok(value)
            }
            None => Ok(DEFAULT_CONTENT_TYPE),
        }
    }
}
