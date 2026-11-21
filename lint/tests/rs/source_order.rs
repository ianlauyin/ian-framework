use std::path::Path;

use crate::source::with_source;
use ian_lint::rule::rs::source_order::check;

const FIX: &str = "Put `mod` and `pub(crate) mod` first, then `use`, then `pub mod`, then `pub use` and `pub(crate) use`, then a private `static`, then constants and private `macro_rules!`, then public items, then private items.";

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

#[test]
fn a_private_macro_before_the_public_items_passes() {
    let source = "\
const LABEL: &str = \"a\";

macro_rules! twice {
    ($value:expr) => {
        $value * 2
    };
}

pub fn run() -> i32 {
    twice!(1)
}
";
    assert!(with_source(Path::new("api/src/name.rs"), source, &[], check).is_empty());
}

#[test]
fn a_private_macro_after_the_public_items_fails() {
    let source = "\
pub fn run() {}

macro_rules! twice {
    ($value:expr) => {
        $value * 2
    };
}
";
    let found = with_source(Path::new("api/src/name.rs"), source, &[], check);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 3);
    assert_eq!(found[0].fix, FIX);
}

#[test]
fn an_item_macro_that_is_not_macro_rules_stays_with_the_private_items() {
    let source = "\
pub fn run() {}

thread_local! {
    static VALUE: i32 = 1;
}
";
    assert!(with_source(Path::new("api/src/name.rs"), source, &[], check).is_empty());
}
