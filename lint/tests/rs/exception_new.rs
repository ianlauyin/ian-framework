use std::path::{Path, PathBuf};

use crate::source::with_source;
use ian_lint::rule::rs::exception_new::{check, check_framework};

fn srcs() -> Vec<PathBuf> {
    vec![PathBuf::from("api/src")]
}

#[test]
fn exception_new_in_exception_rs_passes() {
    let source = "\
fn build() {
    let _ = Exception::__new(1, 2, 3, 4, 5);
}
";
    assert!(
        with_source(
            Path::new("api/src/exception.rs"),
            source,
            &srcs(),
            check_framework,
        )
        .is_empty()
    );
    let other = "\
fn build() {
    Exception::new();
    let _ = \"Exception::__new\";
}
";
    assert!(with_source(Path::new("api/src/app.rs"), other, &srcs(), check).is_empty());
    assert!(with_source(Path::new("api/src/app.rs"), other, &srcs(), check_framework).is_empty());
}

#[test]
fn exception_new_in_exception_rs_fails_on_the_project_ruleset() {
    let source = "\
fn build() {
    Exception::__new(1, 2, 3, 4, 5);
}
";
    let found = with_source(Path::new("api/src/exception.rs"), source, &srcs(), check);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].rule, "exception-new");
    assert_eq!(
        found[0].problem,
        "`Exception::__new` builds an `Exception` outside `exception.rs`."
    );
    assert_eq!(found[0].fix, "Build the `Exception` with `exception!`.");
}

#[test]
fn exception_new_outside_exception_rs_fails() {
    let source = "\
fn build() {
    Exception::__new(1, 2, 3, 4, 5);
    crate::exception::Exception::__new(1, 2, 3, 4, 5);
}
";
    for check_file in [check, check_framework] {
        let found = with_source(Path::new("api/src/app.rs"), source, &srcs(), check_file);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].line, 2);
        assert_eq!(found[0].rule, "exception-new");
        assert_eq!(
            found[0].problem,
            "`Exception::__new` builds an `Exception` outside `exception.rs`."
        );
        assert_eq!(found[0].fix, "Build the `Exception` with `exception!`.");
        assert_eq!(found[1].line, 3);
        assert!(found.iter().all(|item| item.problem == found[0].problem));
        assert!(found.iter().all(|item| item.fix == found[0].fix));
    }
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(with_source(Path::new("api/src/app.rs"), "fn (", &srcs(), check).is_empty());
}

#[test]
fn exception_new_in_a_macro_call_fails() {
    let source = "\
fn build() {
    let _ = vec![Exception::__new(1, 2, 3, 4, 5)];
}
";
    let found = with_source(Path::new("api/src/work.rs"), source, &srcs(), check);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 2);
}
