use google_cloud_storage::model::ListObjectsResponse;

use crate::gcp::storage::ObjectMeta;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListQuery {
    pub prefix: Option<String>,
    pub delimiter: Option<String>,
    pub page_token: Option<String>,
    pub max_results: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPage {
    pub objects: Vec<ObjectMeta>,
    pub prefixes: Vec<String>,
    pub next_page_token: Option<String>,
}

impl ListPage {
    pub(crate) fn from_response(response: ListObjectsResponse) -> Self {
        Self {
            objects: response.objects.into_iter().map(ObjectMeta::from).collect(),
            prefixes: response.prefixes,
            next_page_token: Some(response.next_page_token).filter(|token| !token.is_empty()),
        }
    }
}
