use std::path::{Path, PathBuf};

use crate::source::with_srcs;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::ajax_no_extends::check;

const PATH: &str = "web/src/util/ajax/api/account/logout.ts";

fn diagnostics(path: &str, source: &str) -> Vec<Diagnostic> {
    with_srcs(Path::new(path), source, &[PathBuf::from("web/src")], check)
}

#[test]
fn an_ajax_file_without_extends_passes() {
    let source = "export interface AccountLogoutRequest {\n    code: string;\n}\n";
    assert!(diagnostics(PATH, source).is_empty());
}

#[test]
fn an_interface_that_extends_fails() {
    let source = "export interface AccountLogoutRequest extends Base {}\n";
    assert_eq!(diagnostics(PATH, source).len(), 1);
}

#[test]
fn a_class_that_extends_fails() {
    let source = "class Foo extends Bar {}\n";
    assert_eq!(diagnostics(PATH, source).len(), 1);
}

#[test]
fn a_file_outside_the_ajax_folder_passes() {
    let source = "export interface Foo extends Base {}\n";
    assert!(diagnostics("web/src/util/format.ts", source).is_empty());
}
