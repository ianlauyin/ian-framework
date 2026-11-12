use std::collections::BTreeMap;
use std::pin::Pin;

use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures_util::{Stream, StreamExt};
use serde::Deserialize;

use crate::exception;
use crate::exception::Exception;

const READ_TO_VEC_LIMIT: u64 = 256 * 1024 * 1024;

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
                code = "GCP_TRANSPORT",
                "object response is not bytes",
                source = err.without_url(),
            )
        })?;
        Self::from_slice(&bytes)
    }

    pub(crate) fn from_slice(bytes: &[u8]) -> Result<Self, Exception> {
        let raw: RawObject = serde_json::from_slice(bytes)
            .map_err(|err| exception!(code = "GCP_API", "object JSON is invalid", source = err))?;
        raw.into_meta()
    }
}

pub struct ObjectReader {
    pub(crate) response: reqwest::Response,
}

impl ObjectReader {
    pub fn byte_stream(self) -> Pin<Box<dyn Stream<Item = Result<Bytes, Exception>> + Send>> {
        Box::pin(self.response.bytes_stream().map(|item| {
            item.map_err(|err| {
                exception!(
                    code = "GCP_TRANSPORT",
                    "read object body failed",
                    source = err.without_url(),
                )
            })
        }))
    }

    pub(crate) async fn read_to_vec(self) -> Result<Vec<u8>, Exception> {
        if self
            .response
            .content_length()
            .is_some_and(|length| length > READ_TO_VEC_LIMIT)
        {
            return Err(too_large());
        }
        let mut stream = self.response.bytes_stream();
        let mut body = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|err| {
                exception!(
                    code = "GCP_TRANSPORT",
                    "read object body failed",
                    source = err.without_url(),
                )
            })?;
            if (body.len() + chunk.len()) as u64 > READ_TO_VEC_LIMIT {
                return Err(too_large());
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }
}

impl std::fmt::Debug for ObjectReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectReader")
            .field("status", &self.response.status().as_u16())
            .field("content_length", &self.response.content_length())
            .finish_non_exhaustive()
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
                exception!(code = "GCP_API", "object size is invalid", source = err)
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

fn too_large() -> Exception {
    exception!(
        code = "GCP_CONFIG",
        "object is larger than the 256 MiB limit of get_bytes, so use open_object and byte_stream",
    )
}
