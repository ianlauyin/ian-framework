use serde::Deserialize;

use crate::exception;
use crate::exception::Exception;
use crate::gcp::storage::encode::encode_seg;
use crate::gcp::storage::object::RawObject;
use crate::gcp::storage::ObjectMeta;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListQuery {
    pub prefix: Option<String>,
    pub delimiter: Option<String>,
    pub page_token: Option<String>,
    pub max_results: Option<u32>,
}

impl ListQuery {
    pub(crate) fn url(&self, api_root: &str, bucket_name: &str) -> String {
        let max_results = self.max_results.map(|max| max.to_string());
        let mut pairs: Vec<(&str, &str)> = Vec::new();
        if let Some(delimiter) = self.delimiter.as_deref() {
            pairs.push(("delimiter", delimiter));
        }
        if let Some(max) = max_results.as_deref() {
            pairs.push(("maxResults", max));
        }
        if let Some(token) = self.page_token.as_deref() {
            pairs.push(("pageToken", token));
        }
        if let Some(prefix) = self.prefix.as_deref() {
            pairs.push(("prefix", prefix));
        }
        let mut url = format!("{api_root}/b/{}/o", encode_seg(bucket_name));
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
    pub(crate) fn from_slice(bytes: &[u8]) -> Result<Self, Exception> {
        let raw: RawList = serde_json::from_slice(bytes)
            .map_err(|err| exception!(code = "GCP_API", "list JSON is invalid", source = err))?;
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
