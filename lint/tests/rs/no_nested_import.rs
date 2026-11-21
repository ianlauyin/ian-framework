use std::path::Path;

use crate::source::with_source;
use ian_lint::rule::rs::no_nested_import::check;

#[test]
fn an_import_with_only_names_in_braces_passes() {
    let source = "\
use ian_framework_rs::SessionConfig;
use ian_framework_rs::auth::{SessionConfig as Config, SessionStore};
use std::fmt::*;
";
    assert!(with_source(Path::new("api/src/name.rs"), source, &[], check).is_empty());
}

#[test]
fn an_import_with_a_nested_path_fails() {
    let source = "\
use ian_framework_rs::{auth::{SessionConfig, SessionStore}};
use std::{fmt, path::Path};
use {std::fmt::Debug};

fn sample() {
    use std::{fs::File};
}
";
    let found = with_source(Path::new("api/src/name.rs"), source, &[], check);
    let lines: Vec<_> = found.iter().map(|item| item.line).collect();
    assert_eq!(lines, vec![1, 2, 3, 6]);
    assert_eq!(found[0].rule, "no-nested-import");
    assert_eq!(
        found[0].problem,
        "The braces of this `use` contain a path or braces."
    );
    assert_eq!(
        found[0].fix,
        "Write one `use` for each path. Put only names in the braces."
    );
    assert!(found.iter().all(|item| item.problem == found[0].problem));
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(with_source(Path::new("api/src/name.rs"), "fn (", &[], check).is_empty());
}
