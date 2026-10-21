use serde::Deserialize;

use super::super::encode::encode_seg;
use super::super::{send, Call, GcsBackend, GcsError, GcsErrorKind};
use super::shared::{ListPage, ListQuery, API_ROOT};

impl GcsBackend {
    pub async fn list(&self, query: &ListQuery) -> Result<ListPage, GcsError> {
        let response = send(
            self,
            &Call {
                method: reqwest::Method::GET,
                url: list_url(self.bucket(), query),
                headers: Vec::new(),
                json: None,
                bytes: None,
                extra_status: &[],
                timeout: None,
            },
        )
        .await?;
        let bytes = response.bytes().await.map_err(|err| {
            GcsError::with_source(GcsErrorKind::Transport, "list response is not bytes", err)
        })?;
        list_page_from_slice(&bytes)
    }
}

fn list_url(bucket_name: &str, query: &ListQuery) -> String {
    let max_results = query.max_results.map(|max| max.to_string());
    let mut pairs: Vec<(&str, &str)> = Vec::new();
    if let Some(prefix) = query.prefix.as_deref() {
        pairs.push(("prefix", prefix));
    }
    if let Some(delimiter) = query.delimiter.as_deref() {
        pairs.push(("delimiter", delimiter));
    }
    if let Some(token) = query.page_token.as_deref() {
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

fn list_page_from_slice(bytes: &[u8]) -> Result<ListPage, GcsError> {
    let raw: RawList = serde_json::from_slice(bytes)
        .map_err(|err| GcsError::with_source(GcsErrorKind::Api, "list JSON is invalid", err))?;
    let mut objects = Vec::with_capacity(raw.items.len());
    for item in raw.items {
        objects.push(item.into_meta()?);
    }
    Ok(ListPage {
        objects,
        prefixes: raw.prefixes,
        next_page_token: raw.next_page_token.filter(|token| !token.is_empty()),
    })
}

#[derive(Debug, Deserialize)]
struct RawList {
    #[serde(default)]
    items: Vec<super::shared::RawObject>,
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
        let page = list_page_from_slice(json).unwrap();
        assert_eq!(
            page.prefixes,
            vec![".trash/".to_string(), "photos/".to_string()]
        );
        assert_eq!(page.objects.len(), 1);
        assert!(page.objects[0].metadata.is_empty());
        assert_eq!(page.next_page_token.as_deref(), Some("next"));
    }

    #[test]
    fn list_url_encodes_query_and_does_not_add_a_cap() {
        let url = list_url("buck", &ListQuery::default());
        assert_eq!(url, "https://storage.googleapis.com/storage/v1/b/buck/o");
        let url = list_url(
            "buck",
            &ListQuery {
                prefix: Some("a b".to_string()),
                delimiter: Some("/".to_string()),
                page_token: None,
                max_results: Some(10),
            },
        );
        assert_eq!(
            url,
            "https://storage.googleapis.com/storage/v1/b/buck/o?delimiter=%2F&maxResults=10&prefix=a%20b"
        );
    }
}
