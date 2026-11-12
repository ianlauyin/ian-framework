use crate::gcp::storage::gcs_client::object::list_query::*;
use crate::gcp::storage::API_ROOT;

#[test]
fn list_url_encodes_query_and_does_not_add_a_cap() {
    let url = ListQuery::default().url(API_ROOT, "buck");
    assert_eq!(url, "https://storage.googleapis.com/storage/v1/b/buck/o");
    let url = ListQuery {
        prefix: Some("a b".to_string()),
        delimiter: Some("/".to_string()),
        page_token: None,
        max_results: Some(10),
    }
    .url(API_ROOT, "buck");
    assert_eq!(
        url,
        "https://storage.googleapis.com/storage/v1/b/buck/o?delimiter=%2F&maxResults=10&prefix=a%20b"
    );
}
