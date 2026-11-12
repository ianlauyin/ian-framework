use crate::gcp::response::*;

fn response(body: &str) -> reqwest::Response {
    let response = axum::http::Response::builder()
        .status(200)
        .body(body.to_string())
        .unwrap();
    reqwest::Response::from(response)
}

#[test]
fn google_error_message_reads_both_error_shapes() {
    let body = r#"{"error":{"code":404,"message":"No such object"}}"#;
    assert_eq!(
        google_error_message(body).as_deref(),
        Some("No such object")
    );
    let body = r#"{"error":"invalid_grant","error_description":"Bad Request"}"#;
    assert_eq!(google_error_message(body).as_deref(), Some("Bad Request"));
    assert_eq!(google_error_message("not json"), None);
}

#[test]
fn truncate_keeps_a_char_boundary() {
    assert_eq!(truncate("abc", 5), "abc");
    assert_eq!(truncate("abcdef", 3), "abc");
    assert_eq!(truncate("aé", 2), "a");
}

#[tokio::test]
async fn read_limited_refuses_a_body_over_the_limit() {
    let at_limit = "a".repeat(RESPONSE_BODY_LIMIT);
    let body = read_limited(response(&at_limit), "GCP_API", "test body").await;
    assert_eq!(body.unwrap().len(), RESPONSE_BODY_LIMIT);
    assert_eq!(
        read_limited(response(""), "GCP_API", "test body")
            .await
            .unwrap(),
        b""
    );
    let over = "a".repeat(RESPONSE_BODY_LIMIT + 1);
    let err = read_limited(response(&over), "GCP_AUTH", "test body")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "GCP_AUTH");
    assert_eq!(err.message(), "test body is larger than 256 KiB");
}
