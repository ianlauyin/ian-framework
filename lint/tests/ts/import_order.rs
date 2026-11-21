use std::path::Path;

use crate::source::with_source;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::import_order::check;

const FIX: &str = "Put the other packages first, then `ian-framework-fe`, then the relative paths.";

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    with_source(Path::new("src/a.ts"), source, check)
}

#[test]
fn imports_in_import_order_pass() {
    let source = "\
import { a } from \"solid-js\";
import type { B } from \"@solidjs/web\";

import { c } from \"ian-framework-fe\";
import { d } from \"ian-framework-fe/http\";

import { e } from \"../util\";
import { f } from \"./f\";
import { g } from \".\";

export { h } from \"solid-js\";
export * from \"./i\";
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn imports_out_of_import_order_fail() {
    let source = "\
import { a } from \"./a\";
import { b } from \"ian-framework-fe\";
import { c } from \"solid-js\";
import { d } from \"../d\";
";
    let found = diagnostics(source);
    let lines: Vec<_> = found.iter().map(|item| item.line).collect();
    assert_eq!(lines, vec![2, 3]);
    assert_eq!(found[0].column, 1);
    assert_eq!(found[0].rule, "import-order");
    assert_eq!(found[0].problem, "This import is out of import order.");
    assert_eq!(found[0].fix, FIX);
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}

#[test]
fn a_framework_import_after_a_relative_import_fails() {
    let source = "\
import { a } from \"../a\";
import type { B } from \"ian-framework-fe/http\";
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 2);
}

#[test]
fn a_package_whose_name_starts_with_the_framework_name_is_another_package() {
    let source = "\
import { a } from \"ian-framework-fe\";
import { b } from \"ian-framework-fe-extra\";
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 2);
}

#[test]
fn a_reexport_is_checked_on_its_own() {
    let source = "\
import { a } from \"./a\";
export { b } from \"solid-js\";
export * from \"./c\";
export { d } from \"ian-framework-fe\";
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 4);
}

#[test]
fn a_side_effect_import_is_not_checked() {
    let source = "\
import { a } from \"./a\";
import \"solid-js\";
import { b } from \"./b\";
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn a_file_extension_oxc_does_not_accept_has_no_diagnostic() {
    let source = "import a from \"./a\";\nimport b from \"solid-js\";\n";
    assert!(with_source(Path::new("src/a.css"), source, check).is_empty());
}
