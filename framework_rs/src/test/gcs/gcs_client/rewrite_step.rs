use crate::gcs::gcs_client::rewrite_step::*;
use crate::gcs::API_ROOT;

#[test]
fn rewrite_step_reads_a_token_until_done() {
    let step = RewriteStep::from_slice(br#"{"done":false,"rewriteToken":"abc"}"#).unwrap();
    assert!(matches!(step, RewriteStep::Next(token) if token == "abc"));
    let done =
        RewriteStep::from_slice(br#"{"done":true,"resource":{"name":"b","size":"3"}}"#).unwrap();
    let RewriteStep::Done(resource) = done else {
        panic!("expected a finished rewrite");
    };
    assert_eq!(resource.name, "b");
    assert_eq!(resource.size, 3);
    assert!(RewriteStep::from_slice(br#"{"done":false}"#).is_err());
    assert!(RewriteStep::from_slice(br#"{"done":true}"#).is_err());
    assert!(RewriteStep::from_slice(br#"{"done":false,"rewriteToken":""}"#).is_err());
}

#[test]
fn rewrite_url_sets_the_token_query() {
    assert_eq!(
        RewriteStep::url(API_ROOT, "buck", "a/b", "c d", None),
        "https://storage.googleapis.com/storage/v1/b/buck/o/a%2Fb/rewriteTo/b/buck/o/c%20d"
    );
    let url = RewriteStep::url(API_ROOT, "buck", "a", "c", Some("tok/en"));
    assert!(url.ends_with("?rewriteToken=tok%2Fen"));
}

#[test]
fn rewrite_step_refuses_a_bad_body() {
    assert!(RewriteStep::from_slice(b"not json").is_err());
    let bad_size = br#"{"done":true,"resource":{"name":"b","size":"x"}}"#;
    assert!(RewriteStep::from_slice(bad_size).is_err());
}
