use crate::gcs::gcs_client::{Call, GcsClient};
use crate::gcs::{GcsError, GcsErrorKind, ListPage, ListQuery};

impl GcsClient {
    pub async fn list(&self, query: &ListQuery) -> Result<ListPage, GcsError> {
        let response = self
            .send(&Call {
                method: reqwest::Method::GET,
                url: query.url(self.bucket()),
                ..Default::default()
            })
            .await?;
        let bytes = response.bytes().await.map_err(|err| {
            GcsError::with_source(GcsErrorKind::Transport, "list response is not bytes", err)
        })?;
        ListPage::from_slice(&bytes)
    }
}
