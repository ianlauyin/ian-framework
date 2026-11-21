use std::path::Path;

use crate::rule::diagnostic::Diagnostic;
use crate::rule::rs::file::private_static::check;
use crate::test::parser::rs::source::with_source;

fn lint(source: &str) -> Vec<Diagnostic> {
    with_source(Path::new("src/lib.rs"), source, &[], check)
}

#[test]
fn a_private_static_passes() {
    let source = "\
static VALUE: i32 = 1;
const TEXT: &str = \"pub static VALUE: i32 = 1;\";
fn sample() {
    static LOCAL: i32 = 2;
}
";
    assert!(lint(source).is_empty());
}

#[test]
fn a_public_static_fails() {
    let source = "\
pub static LEFT: i32 = 1;
pub(crate) static RIGHT: i32 = 2;
mod child {
    pub static INNER: i32 = 3;
}
extern \"C\" {
    pub static FOREIGN: i32;
}
";
    let found = lint(source);
    assert_eq!(found.len(), 4);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].column, 1);
    assert_eq!(found[0].rule, "private-static");
    assert_eq!(found[0].problem, "`LEFT` is not a private static.");
    assert_eq!(found[0].fix, "Remove the visibility.");
    assert_eq!(found[1].line, 2);
    assert_eq!(found[1].problem, "`RIGHT` is not a private static.");
    assert_eq!(found[2].line, 4);
    assert_eq!(found[2].column, 5);
    assert_eq!(found[2].problem, "`INNER` is not a private static.");
    assert_eq!(found[3].line, 7);
    assert_eq!(found[3].column, 5);
    assert_eq!(found[3].problem, "`FOREIGN` is not a private static.");
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(lint("pub static").is_empty());
}
