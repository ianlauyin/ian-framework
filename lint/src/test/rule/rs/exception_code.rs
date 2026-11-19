use std::path::Path;

use crate::rule::rs::exception_code::check_source;

#[test]
fn a_constant_error_code_passes() {
    let source = "\
fn build(err: Error) {
    let _ = exception!(code = CODE, \"message\");
    let _ = exception!(severity = Severity::Warn, code = CODE, \"message\", source = err);
    let _ = exception!(code = error_code(&err), \"message\");
    let _ = exception!(code = CODE, \"message\", source = exception!(code = OTHER, \"inner\"));
}
";
    assert!(check_source(Path::new("api/src/app.rs"), source).is_empty());
}

#[test]
fn a_string_literal_error_code_fails() {
    let source = "\
fn build() {
    let _ = exception!(code = \"CODE\", \"message\");
    let _ = exception!(
        severity = Severity::Warn,
        code = \"CODE\",
        \"message\",
    );
    let _ = exception!(code = CODE, \"message\", source = exception!(code = \"INNER\", \"inner\"));
}
";
    let found = check_source(Path::new("api/src/app.rs"), source);
    assert_eq!(found.len(), 3);
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].rule, "exception-code");
    assert_eq!(
        found[0].problem,
        "The error code in `exception!` is a string literal."
    );
    assert_eq!(found[0].fix, "Pass a constant for the error code.");
    assert_eq!(found[1].line, 5);
    assert_eq!(found[2].line, 8);
    assert!(found.iter().all(|item| item.problem == found[0].problem));
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(check_source(Path::new("api/src/app.rs"), "fn (").is_empty());
}
