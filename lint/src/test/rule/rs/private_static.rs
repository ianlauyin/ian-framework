use std::{env, fs, process};

use crate::diagnostic::Diagnostic;
use crate::rule::rs::private_static::check;

fn lint(name: &str, source: &str) -> Vec<Diagnostic> {
    let root = env::temp_dir().join(format!("ian-lint-private-static-{name}-{}", process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old test folder is removed");
    }
    fs::create_dir_all(&root).expect("test folder is created");
    let path = root.join("lib.rs");
    fs::write(&path, source).expect("file is written");
    let found = check(&path);
    fs::remove_dir_all(root).expect("test folder is removed");
    found
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
    assert!(lint("private", source).is_empty());
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
    let found = lint("public", source);
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
    assert!(lint("invalid", "pub static").is_empty());
}

#[test]
fn an_ignore_comment_stops_the_rule() {
    let source = "\
pub static LEFT: i32 = 1; // ian-lint-ignore private-static
// ian-lint-ignore private-static
pub static RIGHT: i32 = 2;
pub static KEPT: i32 = 3;
";
    let found = lint("ignore", source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 4);
    assert_eq!(found[0].problem, "`KEPT` is not a private static.");
}
