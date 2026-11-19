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

#[tokio::test]
async fn a_missing_file_message_contains_no_path() {
    let path = std::env::temp_dir().join(format!("my-app-missing-{}", std::process::id()));
    let error = open_upload_file(&path).await.unwrap_err();
    assert_eq!(error.code(), FILE_OPEN_FAILED);
    assert_eq!(error.message(), "cannot open the file");
}

#[tokio::test]
async fn a_directory_message_contains_no_path() {
    let path = std::env::temp_dir().join(format!("my-app-dir-{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    let error = open_upload_file(&path).await.unwrap_err();
    let _ = std::fs::remove_dir(&path);
    assert_eq!(error.code(), NOT_A_FILE);
    assert_eq!(error.message(), "the path is not a file");
}
