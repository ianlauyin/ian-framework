use crate::gcs::encode::encode_seg;

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
