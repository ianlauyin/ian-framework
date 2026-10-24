use super::encode::encode_seg;
use super::API_ROOT;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
