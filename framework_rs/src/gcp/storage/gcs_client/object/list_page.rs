use serde::Deserialize;

use crate::exception;
use crate::exception::Exception;
use crate::gcp::storage::gcs_client::object::RawObject;
use crate::gcp::storage::gcs_client::ObjectMeta;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPage {
    pub objects: Vec<ObjectMeta>,
    pub prefixes: Vec<String>,
    pub next_page_token: Option<String>,
}

impl ListPage {
    pub(crate) fn from_slice(bytes: &[u8]) -> Result<Self, Exception> {
        let raw: RawList = serde_json::from_slice(bytes)
            .map_err(|err| exception!(code = "GCS_API", "list JSON is invalid", source = err))?;
        Ok(Self {
            objects: raw
                .items
                .into_iter()
                .map(RawObject::into_meta)
                .collect::<Result<_, _>>()?,
            prefixes: raw.prefixes,
            next_page_token: raw.next_page_token.filter(|token| !token.is_empty()),
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawList {
    #[serde(default)]
    items: Vec<RawObject>,
    #[serde(default)]
    prefixes: Vec<String>,
    #[serde(rename = "nextPageToken", default)]
    next_page_token: Option<String>,
}
