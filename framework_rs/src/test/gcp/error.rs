use crate::gcp::error::*;

#[test]
fn api_detail_reads_the_google_message() {
    let body = r#"{"error":{"code":404,"message":"No such object"}}"#;
    assert_eq!(api_detail(body).as_deref(), Some("No such object"));
    let body = r#"{"error":"invalid_grant","error_description":"Bad Request"}"#;
    assert_eq!(api_detail(body).as_deref(), Some("Bad Request"));
    assert_eq!(api_detail("not json"), None);
}

#[test]
fn truncate_keeps_a_char_boundary() {
    assert_eq!(truncate("abc", 5), "abc");
    assert_eq!(truncate("abcdef", 3), "abc");
    assert_eq!(truncate("aé", 2), "a");
}
