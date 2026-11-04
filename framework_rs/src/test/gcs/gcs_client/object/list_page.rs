use crate::gcs::gcs_client::object::list_page::*;

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
