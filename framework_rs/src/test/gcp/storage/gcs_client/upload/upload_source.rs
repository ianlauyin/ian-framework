use crate::gcp::storage::gcs_client::PutBody;

use crate::gcp::storage::gcs_client::upload::upload_source::*;

fn temp_file(name: &str, content: &[u8]) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("my-app-{}-{name}", std::process::id()));
    std::fs::write(&path, content).unwrap();
    path
}

#[tokio::test]
async fn bytes_source_reads_a_range_and_refuses_one_past_the_end() {
    let body = PutBody::Bytes(b"hello".to_vec());
    let mut source = UploadSource::open(&body).await.unwrap();
    assert_eq!(source.len(), 5);
    assert_eq!(source.read_chunk(1, 3).await.unwrap().as_ref(), b"ell");
    let err = source.read_chunk(3, 3).await.unwrap_err();
    assert_eq!(err.code(), "GCS_API");
    let err = source.read_chunk(u64::MAX, usize::MAX).await.unwrap_err();
    assert_eq!(err.code(), "GCS_API");
}

#[tokio::test]
async fn file_source_rereads_any_range() {
    let path = temp_file("source-range", b"hello");
    let body = PutBody::File(path.clone());
    let mut source = UploadSource::open(&body).await.unwrap();
    assert_eq!(source.len(), 5);
    assert_eq!(source.read_chunk(2, 3).await.unwrap().as_ref(), b"llo");
    assert_eq!(source.read_chunk(0, 2).await.unwrap().as_ref(), b"he");
    let err = source.read_chunk(3, 9).await.unwrap_err();
    assert_eq!(err.code(), "GCS_LOCAL");
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn file_source_refuses_a_missing_file_and_a_directory() {
    let missing = PutBody::File(std::env::temp_dir().join("my-app-missing-file"));
    let err = UploadSource::open(&missing).await.map(|_| ()).unwrap_err();
    assert_eq!(err.code(), "GCS_LOCAL");
    let directory = PutBody::File(std::env::temp_dir());
    let err = UploadSource::open(&directory)
        .await
        .map(|_| ())
        .unwrap_err();
    assert_eq!(err.code(), "GCS_CONFIG");
}
