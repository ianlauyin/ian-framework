use std::path::Path;

use crate::source::with_source;
use ian_lint::rule::rs::super_path::check;

#[test]
fn a_crate_path_passes() {
    let source = "\
use super::child::Name;
use crate::child::Name;

fn sample() {
    let _ = \"super::super::Name\";
    super::child();
}
";
    assert!(with_source(Path::new("api/src/child.rs"), source, &[], check).is_empty());
}

#[test]
fn a_super_super_path_fails() {
    let source = "\
use super::super::Name;

fn sample() {
    super::super::child();
}
";
    let found = with_source(Path::new("api/src/child.rs"), source, &[], check);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].rule, "super-path");
    assert_eq!(found[0].problem, "The path uses `super::super`.");
    assert_eq!(found[0].fix, "Use a `crate::` path.");
    assert_eq!(found[1].line, 4);
    assert!(found.iter().all(|item| item.problem == found[0].problem));
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(with_source(Path::new("api/src/child.rs"), "fn (", &[], check).is_empty());
}
