use std::path::Path;

use crate::source::with_source;
use ian_lint::rule::rs::no_self_import::check;

#[test]
fn a_use_without_self_in_braces_passes() {
    let source = "\
use std::fmt;
use std::fmt::{Debug, Display};
use self::child::Inner;
use crate::app::*;
";
    assert!(with_source(Path::new("api/src/app.rs"), source, &[], check).is_empty());
}

#[test]
fn self_in_the_braces_of_a_use_fails() {
    let source = "\
use std::fmt::{self, Debug};
use std::{io::{self as input, Read}, fs};

fn run() {
    use std::path::{self};
}
";
    let found = with_source(Path::new("api/src/app.rs"), source, &[], check);
    let lines: Vec<_> = found.iter().map(|item| item.line).collect();
    assert_eq!(lines, [1, 2, 5]);
    assert_eq!(found[0].column, 16);
    assert_eq!(found[0].rule, "no-self-import");
    assert_eq!(found[0].problem, "The braces of this `use` contain `self`.");
    assert_eq!(found[0].fix, "Import only the module.");
}
