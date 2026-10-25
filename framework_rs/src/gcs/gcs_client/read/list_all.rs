use std::collections::HashSet;

use crate::gcs::gcs_client::GcsClient;
use crate::gcs::{GcsError, GcsErrorKind, ListPage, ListQuery};

impl GcsClient {
    pub async fn list_all(&self, query: &ListQuery) -> Result<ListPage, GcsError> {
        let mut query = query.clone();
        let mut all = ListPage {
            objects: Vec::new(),
            prefixes: Vec::new(),
            next_page_token: None,
        };
        let mut seen_prefixes = HashSet::new();
        let mut seen_tokens = HashSet::new();
        if let Some(token) = &query.page_token {
            seen_tokens.insert(token.clone());
        }
        loop {
            let page = self.list(&query).await?;
            all.objects.extend(page.objects);
            for prefix in page.prefixes {
                if seen_prefixes.insert(prefix.clone()) {
                    all.prefixes.push(prefix);
                }
            }
            let Some(token) = page.next_page_token else {
                return Ok(all);
            };
            if !seen_tokens.insert(token.clone()) {
                return Err(GcsError::new(
                    GcsErrorKind::Api,
                    "list returned a page token that was already used",
                ));
            }
            query.page_token = Some(token);
        }
    }
}
