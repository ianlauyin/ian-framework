use crate::gcp::storage::list::*;
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

#[test]
fn list_keeps_prefixes() {
    let json = br#"{
        "items": [{"name":"keep.txt","size":"3","metadata":null}],
        "prefixes": ["docs/", "photos/"],
        "nextPageToken": "next"
    }"#;
    let page = ListPage::from_slice(json).unwrap();
    assert_eq!(
        page.prefixes,
        vec!["docs/".to_string(), "photos/".to_string()]
    );
    assert_eq!(page.objects.len(), 1);
    assert!(page.objects[0].metadata.is_empty());
    assert_eq!(page.next_page_token.as_deref(), Some("next"));
}
