use serde::Deserialize;

use super::object_meta::RawObject;
use super::{GcsError, GcsErrorKind, ObjectMeta};

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
struct RawList {
    #[serde(default)]
    items: Vec<RawObject>,
    #[serde(default)]
    prefixes: Vec<String>,
    #[serde(rename = "nextPageToken", default)]
    next_page_token: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
