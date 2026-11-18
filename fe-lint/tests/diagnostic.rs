#![allow(clippy::tests_outside_test_module, reason = "An integration test file is a test module.")]

use std::path::Path;

use fe_lint::diagnostic::Diagnostic;

#[test]
fn diagnostic_has_the_fix_line() {
    let diagnostic = Diagnostic::new(Path::new("src/a.css"), 2, 1, "name-case", "`.b` is a second name.", "Rename it to `b-item`.");
    assert_eq!(diagnostic.to_string(), "src/a.css:2:1 code-style/name-case: `.b` is a second name.\n    fix: Rename it to `b-item`.");
}
