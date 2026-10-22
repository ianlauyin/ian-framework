use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::encode::encode_seg;
use super::{GcsError, GcsErrorKind, API_ROOT};

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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListQuery {
    pub prefix: Option<String>,
    pub delimiter: Option<String>,
    pub page_token: Option<String>,
    pub max_results: Option<u32>,
}

impl ListQuery {
    pub(super) fn url(&self, bucket_name: &str) -> String {
        let max_results = self.max_results.map(|max| max.to_string());
        let mut pairs: Vec<(&str, &str)> = Vec::new();
        if let Some(prefix) = self.prefix.as_deref() {
            pairs.push(("prefix", prefix));
        }
        if let Some(delimiter) = self.delimiter.as_deref() {
            pairs.push(("delimiter", delimiter));
        }
        if let Some(token) = self.page_token.as_deref() {
            pairs.push(("pageToken", token));
        }
        if let Some(max) = max_results.as_deref() {
            pairs.push(("maxResults", max));
        }
        pairs.sort_by(|left, right| left.0.cmp(right.0));
        let mut url = format!("{API_ROOT}/b/{}/o", encode_seg(bucket_name));
        if pairs.is_empty() {
            return url;
        }
        url.push('?');
        for (index, (key, value)) in pairs.iter().enumerate() {
            if index > 0 {
                url.push('&');
            }
            url.push_str(&encode_seg(key));
            url.push('=');
            url.push_str(&encode_seg(value));
        }
        url
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPage {
    pub objects: Vec<ObjectMeta>,
    pub prefixes: Vec<String>,
    pub next_page_token: Option<String>,
}

impl ListPage {
    pub(super) fn from_slice(bytes: &[u8]) -> Result<Self, GcsError> {
        let raw: RawList = serde_json::from_slice(bytes)
            .map_err(|err| GcsError::with_source(GcsErrorKind::Api, "list JSON is invalid", err))?;
        let mut objects = Vec::with_capacity(raw.items.len());
        for item in raw.items {
            objects.push(item.into_meta()?);
        }
        Ok(Self {
            objects,
            prefixes: raw.prefixes,
            next_page_token: raw.next_page_token.filter(|token| !token.is_empty()),
        })
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

#[derive(Debug, Deserialize)]
struct RawList {
    #[serde(default)]
    items: Vec<RawObject>,
    #[serde(default)]
    prefixes: Vec<String>,
    #[serde(rename = "nextPageToken", default)]
    next_page_token: Option<String>,
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
    fn put_options_do_not_insert_product_keys() {
        let options = PutOptions::default();
        assert!(options.metadata.is_empty());
        assert_eq!(options.content_type().unwrap(), "application/octet-stream");
    }

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

    #[test]
    fn list_url_encodes_query_and_does_not_add_a_cap() {
        let url = ListQuery::default().url("buck");
        assert_eq!(url, "https://storage.googleapis.com/storage/v1/b/buck/o");
        let url = ListQuery {
            prefix: Some("a b".to_string()),
            delimiter: Some("/".to_string()),
            page_token: None,
            max_results: Some(10),
        }
        .url("buck");
        assert_eq!(
            url,
            "https://storage.googleapis.com/storage/v1/b/buck/o?delimiter=%2F&maxResults=10&prefix=a%20b"
        );
    }

    #[test]
    fn list_keeps_trash_prefixes() {
        let json = br#"{
            "items": [{"name":"keep.txt","size":"3","metadata":null}],
            "prefixes": [".trash/", "photos/"],
            "nextPageToken": "next"
        }"#;
        let page = ListPage::from_slice(json).unwrap();
        assert_eq!(
            page.prefixes,
            vec![".trash/".to_string(), "photos/".to_string()]
        );
        assert_eq!(page.objects.len(), 1);
        assert!(page.objects[0].metadata.is_empty());
        assert_eq!(page.next_page_token.as_deref(), Some("next"));
    }
}
