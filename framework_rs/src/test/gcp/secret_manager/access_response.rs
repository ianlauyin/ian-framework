use crate::gcp::secret_manager::access_response::decode_access_response;
use crate::test::gcp::ok_body;

#[test]
fn decode_gives_the_trimmed_value() {
    assert_eq!(
        decode_access_response(200, &ok_body(b"post-key\n")).unwrap(),
        Some("post-key".to_string())
    );
}

#[test]
fn decode_gives_none_for_404() {
    let body = br#"{"error":{"code":404,"message":"Secret [x] not found"}}"#;
    assert_eq!(decode_access_response(404, body).unwrap(), None);
    assert_eq!(decode_access_response(404, b"").unwrap(), None);
}

#[test]
fn decode_rejects_an_empty_value() {
    for value in [&b""[..], b"  \n"] {
        let err = decode_access_response(200, &ok_body(value)).unwrap_err();
        assert_eq!(err.message(), "secret value is empty");
    }
}

#[test]
fn decode_rejects_a_value_that_is_not_utf8() {
    let err = decode_access_response(200, &ok_body(&[0x66, 0x80, 0x6f])).unwrap_err();
    assert_eq!(err.message(), "secret value is not UTF-8");
}

#[test]
fn decode_rejects_a_bad_body_without_echoing_it() {
    let cases: [&[u8]; 4] = [
        b"not json secret-value",
        br#"{"payload":{}}"#,
        br#"{"payload":{"data":12}}"#,
        br#"{"payload":{"data":"secret-value!"}}"#,
    ];
    for body in cases {
        let err = decode_access_response(200, body).unwrap_err();
        assert_eq!(err.code(), "GCP_API");
        assert!(std::error::Error::source(&err).is_none());
        assert!(!format!("{err} {err:?}").contains("secret-value"), "{err}");
    }
}

#[test]
fn decode_reads_the_api_error() {
    let body =
        br#"{"error":{"code":403,"message":"Permission denied","status":"PERMISSION_DENIED"}}"#;
    let err = decode_access_response(403, body).unwrap_err();
    assert_eq!(err.code(), "GCP_AUTH");
    assert_eq!(
        err.message(),
        "secret manager status 403: Permission denied"
    );
    let err = decode_access_response(500, b"<html>down</html>").unwrap_err();
    assert_eq!(err.code(), "GCP_API");
    assert_eq!(err.message(), "secret manager status 500: ");
}
