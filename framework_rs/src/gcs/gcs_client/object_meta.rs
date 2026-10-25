use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::gcs::{GcsError, GcsErrorKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectMeta {
    pub name: String,
    pub size: u64,
    pub content_type: Option<String>,
    pub updated: Option<DateTime<Utc>>,
    pub metadata: BTreeMap<String, String>,
}

impl ObjectMeta {
    pub(super) async fn from_response(response: reqwest::Response) -> Result<Self, GcsError> {
        let bytes = response.bytes().await.map_err(|err| {
            GcsError::with_source(GcsErrorKind::Transport, "object response is not bytes", err)
        })?;
        Self::from_slice(&bytes)
    }

    fn from_slice(bytes: &[u8]) -> Result<Self, GcsError> {
        let raw: RawObject = serde_json::from_slice(bytes).map_err(|err| {
            GcsError::with_source(GcsErrorKind::Api, "object JSON is invalid", err)
        })?;
        raw.into_meta()
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct RawObject {
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
    pub(super) fn into_meta(self) -> Result<ObjectMeta, GcsError> {
        let size = match self.size {
            Some(text) => text.parse().map_err(|err| {
                GcsError::with_source(GcsErrorKind::Api, "object size is invalid", err)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_json_keeps_caller_metadata() {
        let json = br#"{
            "name": "photos/cat.jpeg",
            "size": "2048",
            "contentType": "image/jpeg",
            "updated": "2020-01-02T03:04:05.678Z",
            "metadata": {"uploaded-by": "ian", "uploaded-by-id": "7"}
        }"#;
        let meta = ObjectMeta::from_slice(json).unwrap();
        assert_eq!(meta.name, "photos/cat.jpeg");
        assert_eq!(meta.size, 2048);
        assert_eq!(meta.content_type.as_deref(), Some("image/jpeg"));
        assert_eq!(
            meta.updated,
            Some(
                DateTime::parse_from_rfc3339("2020-01-02T03:04:05.678Z")
                    .unwrap()
                    .with_timezone(&Utc)
            )
        );
        assert_eq!(
            meta.metadata.get("uploaded-by").map(String::as_str),
            Some("ian")
        );
        assert_eq!(
            meta.metadata.get("uploaded-by-id").map(String::as_str),
            Some("7")
        );
    }

    #[test]
    fn missing_metadata_is_an_empty_map() {
        let json = br#"{"name":"a","size":"1"}"#;
        let meta = ObjectMeta::from_slice(json).unwrap();
        assert!(meta.metadata.is_empty());
        assert_eq!(meta.size, 1);
        assert_eq!(meta.updated, None);
    }
}
