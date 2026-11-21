use std::path::Path;

use crate::source::with_source;
use ian_lint::rule::rs::import_order::check;

const FIX: &str = "Put the other crates first, then `ian_framework_rs::`, then `super::`, `self::`, and the child modules, then `crate::`.";

#[test]
fn imports_in_import_order_pass() {
    let source = "\
use std::fmt;
use serde::Serialize;

use ian_framework_rs::auth::{SessionConfig, SessionStore};
use ian_framework_rs::Exception;

use super::child::Name;

use crate::store::Store;

pub use std::fmt::Debug;
pub use crate::store::Inner;
";
    assert!(with_source(Path::new("api/src/name.rs"), source, &[], check).is_empty());
}

#[test]
fn imports_out_of_import_order_fail() {
    let source = "\
use crate::store::Store;
use super::child::Name;
use ian_framework_rs::Exception;
use std::fmt;
use crate::store::Inner;
";
    let found = with_source(Path::new("api/src/name.rs"), source, &[], check);
    let lines: Vec<_> = found.iter().map(|item| item.line).collect();
    assert_eq!(lines, vec![2, 3, 4]);
    assert_eq!(found[0].rule, "import-order");
    assert_eq!(found[0].problem, "This `use` is out of import order.");
    assert_eq!(found[0].fix, FIX);
    assert!(found.iter().all(|item| item.problem == found[0].problem));
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}

#[test]
fn a_framework_import_after_a_super_import_fails() {
    let source = "\
use super::child::Name;
use ian_framework_rs::Exception;
";
    let found = with_source(Path::new("api/src/name.rs"), source, &[], check);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 2);
}

#[test]
fn an_import_in_an_inline_module_is_checked() {
    let source = "\
mod inner {
    use crate::store::Store;
    use std::fmt;
}
";
    let found = with_source(Path::new("api/src/name.rs"), source, &[], check);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 3);
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(with_source(Path::new("api/src/name.rs"), "fn (", &[], check).is_empty());
}

#[test]
fn a_child_module_import_is_in_the_super_group() {
    let source = "\
mod child;

use std::fmt;
use super::error::Code;
use child::Name;
use self::child::Inner;
use crate::store::Store;
";
    assert!(with_source(Path::new("api/src/name.rs"), source, &[], check).is_empty());
}

#[test]
fn a_child_module_import_after_a_crate_import_fails() {
    let source = "\
mod child;

use crate::store::Store;
use child::Name;
";
    let found = with_source(Path::new("api/src/name.rs"), source, &[], check);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 4);
}
