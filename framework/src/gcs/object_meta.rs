use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::{GcsError, GcsErrorKind};

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
    #[serde(default, deserialize_with = "de_size")]
    size: Option<u64>,
    #[serde(rename = "contentType", default)]
    content_type: Option<String>,
    #[serde(default)]
    updated: Option<String>,
    #[serde(default, deserialize_with = "de_metadata")]
    metadata: BTreeMap<String, String>,
}

impl RawObject {
    pub(super) fn into_meta(self) -> Result<ObjectMeta, GcsError> {
        let updated = match self.updated {
            Some(text) => Some(
                DateTime::parse_from_rfc3339(&text)
                    .map_err(|err| {
                        GcsError::with_source(
                            GcsErrorKind::Api,
                            "object updated time is invalid",
                            err,
                        )
                    })?
                    .with_timezone(&Utc),
            ),
            None => None,
        };
        Ok(ObjectMeta {
            name: self.name,
            size: self.size.unwrap_or(0),
            content_type: self.content_type,
            updated,
            metadata: self.metadata,
        })
    }
}

fn de_size<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Visit;
    impl<'de> serde::de::Visitor<'de> for Visit {
        type Value = Option<u64>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or integer size")
        }

        fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserializer.deserialize_any(Visit)
        }

        fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
            Ok(Some(value))
        }

        fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
            u64::try_from(value).map(Some).map_err(E::custom)
        }

        fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
            if value.is_empty() {
                return Ok(None);
            }
            value.parse::<u64>().map(Some).map_err(E::custom)
        }
    }
    deserializer.deserialize_any(Visit)
}

fn de_metadata<'de, D>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<BTreeMap<String, String>>::deserialize(deserializer)?;
    Ok(value.unwrap_or_default())
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
        let json = br#"{"name":"a","size":1}"#;
        let meta = ObjectMeta::from_slice(json).unwrap();
        assert!(meta.metadata.is_empty());
        assert_eq!(meta.size, 1);
        assert_eq!(meta.updated, None);
    }
}
