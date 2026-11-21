use std::path::Path;

use crate::source::with_source;
use ian_lint::rule::rs::exception_code::check;

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
    assert!(with_source(Path::new("api/src/app.rs"), source, &[], check).is_empty());
}

#[test]
fn a_string_literal_error_code_in_a_test_file_passes() {
    let source = "\
fn build() {
    let _ = exception!(code = \"CODE\", \"message\");
}
";
    let srcs = [std::path::PathBuf::from("api/src")];
    assert!(with_source(Path::new("api/src/tests/app.rs"), source, &srcs, check).is_empty());
    assert!(with_source(Path::new("api/tests/app.rs"), source, &srcs, check).is_empty());
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
    let found = with_source(Path::new("api/src/app.rs"), source, &[], check);
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
    assert!(with_source(Path::new("api/src/app.rs"), "fn (", &[], check).is_empty());
}

#[test]
fn a_string_literal_error_code_in_another_macro_call_fails() {
    let source = "\
fn build() {
    let _ = vec![exception!(code = \"CODE\", \"message\")];
    exception!(code = \"CODE\", \"message\");
}
";
    let lines: Vec<_> = with_source(Path::new("api/src/app.rs"), source, &[], check)
        .into_iter()
        .map(|item| item.line)
        .collect();
    assert_eq!(lines, vec![2, 3]);
}
