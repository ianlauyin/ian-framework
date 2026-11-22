use std::path::Path;

use crate::source::with_source;
use ian_lint::rule::rs::no_framework_alias::check;

#[test]
fn a_framework_import_with_no_alias_passes() {
    let source = "\
use ian_framework_rs::http::middleware;
use ian_framework_rs::exception::{Exception, Severity};
use ian_framework_rs::logging::Layer as _;
use axum::middleware as layers;
";
    assert!(with_source(Path::new("api/src/app.rs"), source, &[], check).is_empty());
}

#[test]
fn a_framework_import_with_an_alias_fails() {
    let source = "\
use ian_framework_rs::http::middleware as framework_middleware;
use ian_framework_rs::exception::{Exception as Error, Severity};
";
    let found = with_source(Path::new("api/src/app.rs"), source, &[], check);
    let lines: Vec<_> = found.iter().map(|item| item.line).collect();
    assert_eq!(lines, [1, 2]);
    assert_eq!(found[0].rule, "no-framework-alias");
    assert_eq!(
        found[0].problem,
        "This `use` gives an alias to an `ian_framework_rs` item."
    );
    assert_eq!(found[0].fix, "Import the item with its name.");
}
