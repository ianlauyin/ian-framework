use crate::gcp::secret_manager::*;

#[test]
fn a_secret_value_loses_its_surrounding_whitespace() {
    assert_eq!(parse_secret_value(b" value\n").unwrap(), "value");
}

#[test]
fn a_secret_value_must_be_text_and_not_empty() {
    let not_utf8 = parse_secret_value(&[0xff, 0xfe]).unwrap_err();
    assert_eq!(not_utf8.code(), "GCP_CONFIG");
    let empty = parse_secret_value(b" \n").unwrap_err();
    assert_eq!(empty.code(), "GCP_CONFIG");
}
