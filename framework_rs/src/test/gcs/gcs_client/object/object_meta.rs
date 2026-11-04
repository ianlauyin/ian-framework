use chrono::{DateTime, Utc};

use crate::gcs::gcs_client::object::object_meta::*;

#[test]
fn object_json_keeps_caller_metadata() {
    let json = br#"{
        "name": "photos/cat.jpeg",
        "size": "2048",
        "contentType": "image/jpeg",
        "updated": "2020-01-02T03:04:05.678Z",
        "metadata": {"owner": "alice", "owner-id": "7"}
    }"#;
    let meta = ObjectMeta::from_slice(json).unwrap();
    assert_eq!(meta.name, "photos/cat.jpeg");
    assert_eq!(meta.size, 2048);
    assert_eq!(meta.content_type.as_deref(), Some("image/jpeg"));
    assert_eq!(
        meta.updated,
        Some(
            DateTime::parse_from_rfc3339("2020-01-02T03:04:05.678Z")
                .unwrap()
                .with_timezone(&Utc)
        )
    );
    assert_eq!(
        meta.metadata.get("owner").map(String::as_str),
        Some("alice")
    );
    assert_eq!(meta.metadata.get("owner-id").map(String::as_str), Some("7"));
}
