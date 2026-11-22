use std::path::{Path, PathBuf};

use crate::source::with_source;
use ian_lint::rule::rs::test_crate_path::check;

fn srcs() -> Vec<PathBuf> {
    vec![PathBuf::from("api/src")]
}

const SOURCE: &str = "\
use super::helper;
use super::super::app::run;

pub(super) fn sample() {
    let _ = super::other();
    assert!(super::ready());
}
";

#[test]
fn a_super_path_in_a_test_file_fails() {
    for path in ["api/src/tests/app.rs", "api/tests/app.rs"] {
        let found = with_source(Path::new(path), SOURCE, &srcs(), check);
        let lines: Vec<_> = found.iter().map(|item| item.line).collect();
        assert_eq!(lines, [1, 2, 5, 6]);
        assert_eq!(found[0].rule, "test-crate-path");
        assert_eq!(found[0].problem, "A test file uses a `super` path.");
        assert_eq!(found[0].fix, "Use a `crate::` path.");
    }
}

#[test]
fn a_super_path_in_a_source_file_passes() {
    assert!(with_source(Path::new("api/src/app.rs"), SOURCE, &srcs(), check).is_empty());
}

#[test]
fn a_crate_path_in_a_test_file_passes() {
    let source = "\
use crate::app::run;
use crate::tests::helper;

pub(super) fn sample() {
    let _ = crate::app::other();
}
";
    assert!(with_source(Path::new("api/src/tests/app.rs"), source, &srcs(), check).is_empty());
}
