#[test]
fn a_tls_url_is_accepted() {
    assert!(redis::Client::open("rediss://default:secret@redis.example.com:6379").is_ok());
}
