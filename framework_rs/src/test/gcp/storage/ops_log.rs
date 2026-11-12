use crate::gcp::storage::ops_log::*;

#[test]
fn logged_error_omits_the_query() {
    let text = logged_error(
        &"error sending request for url (https://storage.googleapis.com/o?upload_id=secret): connection reset",
    );
    assert!(!text.contains("secret"));
    assert!(!text.contains("upload_id"));
    assert!(text.contains("connection reset"));
}

#[test]
fn path_only_omits_the_query() {
    assert_eq!(
        path_only(
            "https://storage.googleapis.com/upload/storage/v1/b/bucket/o?uploadType=resumable&upload_id=secret"
        ),
        "/upload/storage/v1/b/bucket/o"
    );
    assert_eq!(path_only("/obj?upload_id=secret"), "/obj");
}
