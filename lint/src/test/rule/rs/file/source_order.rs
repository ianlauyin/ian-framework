use std::path::Path;

use crate::rule::edit::apply;
use crate::rule::rs::file::source_order::{check, edit};
use crate::test::rule::rs::source::with_source;

const FIX: &str = "Put `mod` and `pub(crate) mod` first, then `use`, then `pub mod`, then `pub use` and `pub(crate) use`, then a private `static`, then constants, then public items, then private items.";

#[test]
fn items_in_source_order_pass() {
    let source = "\
mod child;
pub(crate) mod hidden;

use std::fmt;

pub mod open;

pub use open::Name;
pub(crate) use hidden::Inner;

static VALUE: i32 = 1;

const LABEL: &str = \"a\";
pub const CODE: &str = \"b\";

pub struct Name;

impl Name {
    pub fn new() -> Self {
        Self
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(\"name\")
    }
}

fn helper() {}
";
    assert!(with_source(Path::new("api/src/name.rs"), source, &[], check).is_empty());
}

#[test]
fn an_item_out_of_source_order_fails() {
    let source = "\
pub struct Name;

use std::fmt;

const LABEL: &str = \"a\";

fn helper() {}

pub fn run() {}
";
    let found = with_source(Path::new("api/src/name.rs"), source, &[], check);
    let lines: Vec<_> = found.iter().map(|item| item.line).collect();
    assert_eq!(lines, vec![3, 5, 9]);
    assert_eq!(found[0].rule, "source-order");
    assert_eq!(found[0].problem, "This item is out of source order.");
    assert_eq!(found[0].fix, FIX);
    assert!(found.iter().all(|item| item.problem == found[0].problem));
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(with_source(Path::new("api/src/name.rs"), "fn (", &[], check).is_empty());
}

fn formatted(source: &str) -> String {
    with_source(Path::new("api/src/name.rs"), source, &[], |file| {
        let found = edit(file);
        let refs: Vec<_> = found.iter().collect();
        apply(source, &refs).expect("edits apply")
    })
}

#[test]
fn an_item_out_of_source_order_is_formatted() {
    let source = "\
pub struct Name;

use std::fmt;

fn helper() {}

pub fn run() {}
";
    let next = formatted(source);
    assert!(with_source(Path::new("api/src/name.rs"), &next, &[], check).is_empty());
    let use_at = next.find("use std::fmt;").unwrap();
    let struct_at = next.find("pub struct Name;").unwrap();
    let public_fn = next.find("pub fn run()").unwrap();
    let private_fn = next.find("fn helper()").unwrap();
    assert!(use_at < struct_at);
    assert!(public_fn < private_fn);
}

#[test]
fn items_in_source_order_are_unchanged_by_the_edit() {
    let source = "\
use std::fmt;

pub struct Name;

pub fn run() {}

fn helper() {}
";
    assert_eq!(formatted(source), source);
}

#[test]
fn an_attribute_stays_with_its_item() {
    let source = "\
#[allow(dead_code)]
fn helper() {}

pub fn run() {}
";
    let next = formatted(source);
    assert!(with_source(Path::new("api/src/name.rs"), &next, &[], check).is_empty());
    let attribute = next.find("#[allow(dead_code)]").unwrap();
    let helper = next.find("fn helper()").unwrap();
    let run = next.find("pub fn run()").unwrap();
    assert!(run < attribute);
    assert!(attribute < helper);
}

#[test]
fn a_nested_module_is_inside_the_edit() {
    let source = "\
fn helper() {}

mod child {
    fn b() {}

    use std::fmt;
}
";
    let next = formatted(source);
    assert!(with_source(Path::new("api/src/name.rs"), &next, &[], check).is_empty());
    assert!(next.find("mod child").unwrap() < next.find("fn helper()").unwrap());
    let module = next.find("mod child").unwrap();
    let use_at = next.find("use std::fmt;").unwrap();
    let inner_fn = next.find("fn b()").unwrap();
    assert!(module < use_at);
    assert!(use_at < inner_fn);
    let found = with_source(Path::new("api/src/name.rs"), source, &[], edit);
    assert_eq!(found.len(), 1);
}
