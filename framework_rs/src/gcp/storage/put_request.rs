use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::exception;
use crate::exception::Exception;

pub const CONTENT_TYPE_INVALID: &str = "CONTENT_TYPE_INVALID";
pub const FILE_METADATA_FAILED: &str = "FILE_METADATA_FAILED";
pub const FILE_OPEN_FAILED: &str = "FILE_OPEN_FAILED";
pub const NOT_A_FILE: &str = "NOT_A_FILE";

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

pub(crate) async fn open_upload_file(path: &Path) -> Result<tokio::fs::File, Exception> {
    let file = tokio::fs::File::open(path).await.map_err(|err| {
        exception!(
            code = FILE_OPEN_FAILED,
            format!("cannot open {}", path.display()),
            source = err,
        )
    })?;
    let meta = file.metadata().await.map_err(|err| {
        exception!(
            code = FILE_METADATA_FAILED,
            format!("cannot read metadata for {}", path.display()),
            source = err,
        )
    })?;
    if !meta.is_file() {
        return Err(exception!(
            code = NOT_A_FILE,
            format!("{} is not a file", path.display()),
        ));
    }
    Ok(file)
}
