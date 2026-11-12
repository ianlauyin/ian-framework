use crate::exception;

use crate::gcp::storage::error::*;
#[test]
fn response_detail_has_no_url_query_or_control_character() {
    let body = "bad url https://storage.example/o?X-Goog-Signature=abc&token=xyz now\r\nnext";
    let detail = response_detail(body);
    assert_eq!(detail, "bad url https://storage.example/o now  next");
    let json = r#"{"error":{"message":"see https://h/o?sig=secret"}}"#;
    assert_eq!(response_detail(json), "see https://h/o");
    assert_eq!(response_detail(&"é".repeat(500)).len(), 200);
}

#[test]
fn redact_query_keeps_plain_text_with_a_question_mark() {
    for text in [
        "Invalid argument?name=foo",
        "is it correct? more text",
        "what?",
    ] {
        assert_eq!(redact_query(text), text);
    }
}

#[test]
fn redact_query_cuts_only_the_url_query_in_compact_json() {
    let json = r#"{"url":"https://h/o?sig=secret&a=1","next":"ok?x"}"#;
    assert_eq!(redact_query(json), r#"{"url":"https://h/o","next":"ok?x"}"#);
    let text = "failed (HTTPS://h/o?upload_id=secret): reset";
    assert_eq!(redact_query(text), "failed (HTTPS://h/o): reset");
}

#[test]
fn redact_query_hides_a_signed_parameter_without_a_url() {
    let text = "bad X-Goog-Signature=abc&X-Goog-Expires=60 done";
    let redacted = redact_query(text);
    assert_eq!(redacted, "bad X-Goog-Signature=***&X-Goog-Expires=*** done");
}

#[test]
fn redact_query_cuts_the_query_of_a_relative_or_scheme_less_url() {
    for (text, expected) in [
        (
            "GET /upload/o?upload_id=secret failed",
            "GET /upload/o failed",
        ),
        (
            "see storage.example/b/o?sig=secret now",
            "see storage.example/b/o now",
        ),
        ("path=/o?x=1,next", "path=/o,next"),
    ] {
        assert_eq!(redact_query(text), expected);
    }
}

#[test]
fn redact_query_hides_a_known_secret_key_without_a_query() {
    assert_eq!(
        redact_query("denied access_token=ya29.abc and upload_id=xyz"),
        "denied access_token=*** and upload_id=***"
    );
    assert_eq!(redact_query("sig=abc&keep=1"), "sig=***&keep=1");
    assert_eq!(
        redact_query("Authorization: Bearer ya29.abc failed"),
        "Authorization: Bearer *** failed"
    );
    assert_eq!(redact_query("bearer abc"), "bearer ***");
}

#[test]
fn redact_query_ends_a_token_at_a_comma_or_a_semicolon() {
    assert_eq!(
        redact_query("a=https://h/o?sig=secret;b=2,c=3]d}"),
        "a=https://h/o;b=2,c=3]d}"
    );
    assert_eq!(redact_query("token=abc,next=1"), "token=***,next=1");
    assert_eq!(redact_query("[\"https://h/o?sig=s\"]"), "[\"https://h/o\"]");
}

#[test]
fn redact_query_keeps_prose() {
    for text in [
        "is it correct? more text",
        "Invalid argument?name=foo",
        "what? yes, no; maybe",
        "the token was rejected",
    ] {
        assert_eq!(redact_query(text), text);
    }
}

#[test]
fn retryable_codes_match_the_upload_policy() {
    assert!(is_retryable(&exception!(code = "GCP_TRANSPORT", "reset")));
    assert!(is_retryable(&exception!(code = "GCS_UNAVAILABLE", "busy")));
    for code in [
        "GCS_LOCAL",
        "GCS_NOT_FOUND",
        "GCP_API",
        "GCP_CONFIG",
        "GCP_AUTH",
    ] {
        assert!(!is_retryable(&exception!(code = code, "x")), "{code}");
    }
}
