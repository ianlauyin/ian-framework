mod parse;
pub(crate) mod rule_empty_line_before;
pub(crate) mod selector_nested_pattern;

use std::path::Path;

use crate::diagnostic::Diagnostic;

pub(crate) fn rule_empty_line_before(path: &Path) -> Vec<Diagnostic> {
    rule_empty_line_before::check(path)
}

pub(crate) fn selector_nested_pattern(path: &Path) -> Vec<Diagnostic> {
    selector_nested_pattern::check(path)
}
