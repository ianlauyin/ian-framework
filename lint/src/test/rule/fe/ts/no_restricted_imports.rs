use std::path::Path;

use crate::rule::diagnostic::Diagnostic;
use crate::rule::fe::ts::file::with_source;
use crate::rule::fe::ts::no_restricted_imports::check;

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    with_source(Path::new("src/a.ts"), source, check)
}

#[test]
fn a_package_import_passes() {
    let source = "\
import { x } from \"solid-js\";
import y from \"solid-js/web\";
export { x };
export const text = \"node_modules/pkg\";
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn an_import_with_one_segment_after_node_modules_fails() {
    let source = "\
import a from \"node_modules/pkg\";
import b from \"./node_modules/pkg\";
import c from \"../node_modules/pkg\";
import d from \"foo/node_modules/bar\";
export { e } from \"node_modules/pkg\";
export * from \"foo/node_modules/bar\";
import type f from \"node_modules/pkg\";
import g from \"node_modules\\/pkg\";
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 8);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].column, 15);
    assert_eq!(found[0].rule, "no-restricted-imports");
    assert_eq!(
        found[0].problem,
        "`node_modules/pkg` goes through `node_modules`."
    );
    assert_eq!(found[0].fix, "Import the package by its name.");
    assert!(found.iter().all(|item| item.fix == found[0].fix));
    assert_eq!(
        found[7].problem,
        "`node_modules/pkg` goes through `node_modules`."
    );
}

#[test]
fn an_import_with_two_segments_after_node_modules_passes() {
    let source = "\
import a from \"node_modules/pkg/sub\";
import b from \"foo/node_modules/bar/baz\";
import c from \"node_modules\";
import d from \"node_modules/pkg/\";
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn an_import_call_and_an_import_equals_require_fail() {
    let source = "\
export const a = import(\"node_modules/pkg\");
import b = require(\"node_modules/pkg\");
import type c = require(\"foo/node_modules/bar\");
export const d = import(/* use the package name */ \"node_modules/pkg\");
";
    let lines: Vec<_> = diagnostics(source)
        .into_iter()
        .map(|item| item.line)
        .collect();
    assert_eq!(lines, vec![1, 2, 3, 4]);
}

#[test]
fn a_require_call_and_a_string_that_is_not_an_import_pass() {
    let source = "\
export const a = require(\"node_modules/pkg\");
export const b = import(`node_modules/pkg`);
export const c = import(`node_modules/${name}`);
export const d = obj.import(\"node_modules/pkg\");
export const e = obj?.import(\"node_modules/pkg\");
// import f from \"node_modules/pkg\";
/* import g from \"node_modules/pkg\"; */
const text = \"import h from \\\"node_modules/pkg\\\"\";
const pattern = /from \"node_modules\\/pkg\"/;
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn jsx_text_is_not_an_import() {
    let source = "\
export const view = <div>import a from \"node_modules/pkg\"</div>;
import b from \"node_modules/pkg\";
";
    let found = with_source(Path::new("src/a.tsx"), source, check);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 2);
}

#[test]
fn a_file_extension_oxc_does_not_accept_has_no_diagnostic() {
    let source = "import a from \"node_modules/pkg\";\n";
    assert!(with_source(Path::new("src/a.css"), source, check).is_empty());
}
