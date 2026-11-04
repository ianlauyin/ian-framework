use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::exception;
use crate::exception::Exception;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectMeta {
    pub name: String,
    pub size: u64,
    pub content_type: Option<String>,
    pub updated: Option<DateTime<Utc>>,
    pub metadata: BTreeMap<String, String>,
}

impl ObjectMeta {
    pub(crate) async fn from_response(response: reqwest::Response) -> Result<Self, Exception> {
        let bytes = response.bytes().await.map_err(|err| {
            exception!(
                "object response is not bytes",
                code = "GCS_TRANSPORT",
                source = err.without_url()
            )
        })?;
        Self::from_slice(&bytes)
    }

    pub(crate) fn from_slice(bytes: &[u8]) -> Result<Self, Exception> {
        let raw: RawObject = serde_json::from_slice(bytes)
            .map_err(|err| exception!("object JSON is invalid", code = "GCS_API", source = err))?;
        raw.into_meta()
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawObject {
    name: String,
    #[serde(default)]
    size: Option<String>,
    #[serde(rename = "contentType", default)]
    content_type: Option<String>,
    #[serde(default)]
    updated: Option<DateTime<Utc>>,
    #[serde(default)]
    metadata: Option<BTreeMap<String, String>>,
}

impl RawObject {
    pub(crate) fn into_meta(self) -> Result<ObjectMeta, Exception> {
        let size = match self.size {
            Some(text) => text.parse().map_err(|err| {
                exception!("object size is invalid", code = "GCS_API", source = err)
            })?,
            None => 0,
        };
        Ok(ObjectMeta {
            name: self.name,
            size,
            content_type: self.content_type,
            updated: self.updated,
            metadata: self.metadata.unwrap_or_default(),
        })
    }
}
