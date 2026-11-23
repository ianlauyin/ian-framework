use ian_framework_rs::gcp::storage::public_url::*;

#[test]
fn a_public_url_keeps_the_slash_and_encodes_the_rest() {
    assert_eq!(
        public_url("my-bucket", "account/1/logo"),
        "https://storage.googleapis.com/my-bucket/account/1/logo"
    );
    assert_eq!(
        public_url("my-bucket", "a b/\u{e9}.png"),
        "https://storage.googleapis.com/my-bucket/a%20b/%C3%A9.png"
    );
}
