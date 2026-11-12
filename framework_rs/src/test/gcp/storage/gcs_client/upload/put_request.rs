use crate::gcp::storage::gcs_client::upload::put_request::*;

#[test]
fn put_options_do_not_insert_product_keys() {
    let options = PutOptions::default();
    assert!(options.metadata.is_empty());
    assert_eq!(options.content_type().unwrap(), "application/octet-stream");
}

#[test]
fn content_type_must_be_printable_ascii() {
    let with = |value: &str| PutOptions {
        content_type: Some(value.to_string()),
        ..PutOptions::default()
    };
    assert_eq!(with("text/plain").content_type().unwrap(), "text/plain");
    assert!(with("").content_type().is_err());
    assert!(with("text/é").content_type().is_err());
    assert!(with("text/plain\n").content_type().is_err());
}
