use crate::gcp::storage::signed_url::encode_path;

pub fn public_url(bucket: &str, object: &str) -> String {
    format!(
        "https://storage.googleapis.com/{bucket}/{}",
        encode_path(object)
    )
}
