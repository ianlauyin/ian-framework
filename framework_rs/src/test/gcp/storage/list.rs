use google_cloud_storage::model::{ListObjectsResponse, Object};

use crate::gcp::storage::list::*;

#[test]
fn an_empty_page_token_means_the_last_page() {
    let last = ListPage::from_response(ListObjectsResponse::new());
    assert_eq!(last.next_page_token, None);
    let more = ListPage::from_response(ListObjectsResponse::new().set_next_page_token("next"));
    assert_eq!(more.next_page_token.as_deref(), Some("next"));
}

#[test]
fn a_page_keeps_the_objects_and_the_prefixes() {
    let response = ListObjectsResponse::new()
        .set_objects([Object::new().set_name("a/b.txt").set_size(3)])
        .set_prefixes(["a/"]);
    let page = ListPage::from_response(response);
    assert_eq!(page.prefixes, ["a/"]);
    assert_eq!(page.objects.len(), 1);
    assert_eq!(page.objects[0].name, "a/b.txt");
    assert_eq!(page.objects[0].size, 3);
}
