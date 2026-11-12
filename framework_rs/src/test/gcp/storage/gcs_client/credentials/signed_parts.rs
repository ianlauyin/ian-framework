use std::time::Duration;

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

use crate::gcp::storage::gcs_client::credentials::signed_parts::*;
use crate::test::gcp::storage::gcs_client::{fake_client, serve_script, Reply};

#[test]
fn v4_get_matches_the_canonical_request() {
    let now = DateTime::parse_from_rfc3339("2018-10-26T21:19:42Z")
        .unwrap()
        .with_timezone(&Utc);
    let parts = SignedParts::v4_get(
        "example-bucket",
        "photos/cat.jpeg",
        "signer@example-project.iam.gserviceaccount.com",
        now,
        Some(Duration::from_secs(3600)),
        None,
    )
    .unwrap();
    let canonical = "\
GET
/example-bucket/photos/cat.jpeg
X-Goog-Algorithm=GOOG4-RSA-SHA256&X-Goog-Credential=signer%40example-project.iam.gserviceaccount.com%2F20181026%2Fauto%2Fstorage%2Fgoog4_request&X-Goog-Date=20181026T211942Z&X-Goog-Expires=3600&X-Goog-SignedHeaders=host
host:storage.googleapis.com

host
UNSIGNED-PAYLOAD";
    let hash = hex::encode(Sha256::digest(canonical.as_bytes()));
    assert_eq!(
        hash,
        "efdb769ae1f6dc9f5fc558b88db91be5e7dc5abec2dbb8a135ea7ea85bd8fdd4"
    );
    let expected_sts =
        format!("GOOG4-RSA-SHA256\n20181026T211942Z\n20181026/auto/storage/goog4_request\n{hash}");
    assert_eq!(parts.string_to_sign, expected_sts);
    assert_eq!(
        parts.url_prefix,
        "https://storage.googleapis.com/example-bucket/photos/cat.jpeg?X-Goog-Algorithm=GOOG4-RSA-SHA256&X-Goog-Credential=signer%40example-project.iam.gserviceaccount.com%2F20181026%2Fauto%2Fstorage%2Fgoog4_request&X-Goog-Date=20181026T211942Z&X-Goog-Expires=3600&X-Goog-SignedHeaders=host"
    );
}

#[test]
fn v4_encodes_spaces_in_the_object_path() {
    let now = DateTime::parse_from_rfc3339("2018-10-26T21:19:42Z")
        .unwrap()
        .with_timezone(&Utc);
    let parts = SignedParts::v4_get(
        "b",
        "my cat.jpeg",
        "s@example.iam.gserviceaccount.com",
        now,
        Some(Duration::from_secs(60)),
        None,
    )
    .unwrap();
    assert!(parts.url_prefix.contains("/b/my%20cat.jpeg?"));
    assert!(parts
        .string_to_sign
        .contains("20181026/auto/storage/goog4_request"));
}

#[test]
fn object_path_keeps_slashes() {
    assert_eq!(
        encode_object_path("photos/my cat.jpeg"),
        "photos/my%20cat.jpeg"
    );
    assert_eq!(encode_object_path("a//b"), "a//b");
}

#[test]
fn v4_signs_the_attachment_disposition_after_the_goog_params() {
    let now = DateTime::parse_from_rfc3339("2018-10-26T21:19:42Z")
        .unwrap()
        .with_timezone(&Utc);
    let disposition = attachment_disposition("報告 \"v2\".pdf").unwrap();
    assert_eq!(
        disposition,
        "attachment; filename=\"__ _v2_.pdf\"; filename*=UTF-8''%E5%A0%B1%E5%91%8A%20%22v2%22.pdf"
    );
    let parts = SignedParts::v4_get(
        "b",
        "a.pdf",
        "s@example.iam.gserviceaccount.com",
        now,
        Some(Duration::from_secs(60)),
        Some("報告 \"v2\".pdf"),
    )
    .unwrap();
    assert!(parts.url_prefix.ends_with(
        "&X-Goog-SignedHeaders=host&response-content-disposition=attachment%3B%20filename%3D%22__%20_v2_.pdf%22%3B%20filename%2A%3DUTF-8%27%27%25E5%25A0%25B1%25E5%2591%258A%2520%2522v2%2522.pdf"
    ));
    let without = SignedParts::v4_get(
        "b",
        "a.pdf",
        "s@example.iam.gserviceaccount.com",
        now,
        Some(Duration::from_secs(60)),
        None,
    )
    .unwrap();
    assert_ne!(parts.string_to_sign, without.string_to_sign);
}

#[test]
fn attachment_disposition_rejects_an_empty_filename() {
    let err = attachment_disposition("").unwrap_err();
    assert_eq!(err.code(), "GCS_CONFIG");
}

#[test]
fn signed_lifetime_never_returns_zero() {
    for bad in [
        None,
        Some(Duration::ZERO),
        Some(Duration::from_millis(999)),
        Some(Duration::from_secs(MAX_SIGNED_URL_SECONDS + 1)),
    ] {
        assert_eq!(
            signed_lifetime_seconds(bad).unwrap_err().code(),
            "GCS_CONFIG"
        );
    }
    assert_eq!(
        signed_lifetime_seconds(Some(Duration::from_secs(1))).unwrap(),
        1
    );
}

#[tokio::test]
async fn sign_blob_returns_the_decoded_signature() {
    let reply = Reply::new(200, r#"{"signedBlob":"AQ ID\n"}"#);
    let (addr, server) = serve_script(vec![reply]).await;
    let signature = sign_blob(&fake_client(addr), b"payload").await.unwrap();
    assert_eq!(signature, [1, 2, 3]);
    let request = server.await.unwrap().remove(0);
    assert!(request.starts_with(
        "POST /iam/projects/-/serviceAccounts/signer%40example.iam.gserviceaccount.com:signBlob"
    ));
    assert!(
        request.ends_with(r#"{"payload":"cGF5bG9hZA=="}"#),
        "{request}"
    );
}

#[tokio::test]
async fn signed_url_ends_with_the_hex_signature() {
    let (addr, _server) = serve_script(vec![Reply::new(200, r#"{"signedBlob":"AQID"}"#)]).await;
    let url = fake_client(addr)
        .signed_get_url("a/b.txt", Some(Duration::from_secs(60)), None)
        .await
        .unwrap();
    assert!(url.ends_with("&X-Goog-Signature=010203"), "{url}");
}

#[tokio::test]
async fn sign_blob_reports_a_failed_call() {
    let (addr, _server) = serve_script(vec![Reply::new(403, "{}")]).await;
    let err = sign_blob(&fake_client(addr), b"p").await.unwrap_err();
    assert_eq!(err.code(), "GCS_AUTH");
}

#[test]
fn a_bad_sign_blob_body_is_an_api_error() {
    for body in [
        &b"not json"[..],
        br#"{"signedBlob":"%%%"}"#,
        br#"{"signedBlob":""}"#,
    ] {
        let err = signature_from_response(body).unwrap_err();
        assert_eq!(err.code(), "GCS_API");
    }
}
