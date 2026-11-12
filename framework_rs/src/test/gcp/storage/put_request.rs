use crate::gcp::storage::put_request::*;

fn options(content_type: Option<&str>) -> PutOptions {
    PutOptions {
        content_type: content_type.map(str::to_string),
        ..PutOptions::default()
    }
}

#[test]
fn the_default_content_type_is_a_byte_stream() {
    assert_eq!(
        options(None).content_type_or_default().unwrap(),
        "application/octet-stream"
    );
}

#[test]
fn a_content_type_must_be_printable_ascii_and_not_empty() {
    assert_eq!(
        options(Some("text/plain"))
            .content_type_or_default()
            .unwrap(),
        "text/plain"
    );
    for bad in ["", "text/\nplain", "text/\u{e9}"] {
        assert!(options(Some(bad)).content_type_or_default().is_err());
    }
}
