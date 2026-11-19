mod parse;
pub(crate) mod rule_empty_line_before;
pub(crate) mod selector_nested_pattern;

use std::path::Path;

use crate::diagnostic::Diagnostic;

pub fn check(path: &Path) -> Vec<Diagnostic> {
    let mut diagnostics = rule_empty_line_before::check(path);
    diagnostics.extend(selector_nested_pattern::check(path));
    diagnostics
}
