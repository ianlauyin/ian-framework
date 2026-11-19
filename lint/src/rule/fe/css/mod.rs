mod parse;
pub(crate) mod rule_empty_line_before;
pub(crate) mod selector_nested_pattern;

use std::path::Path;

use crate::diagnostic::Diagnostic;

pub(crate) fn empty_line(path: &Path) -> Vec<Diagnostic> {
    rule_empty_line_before::check(path)
}

pub(crate) fn nested(path: &Path) -> Vec<Diagnostic> {
    selector_nested_pattern::check(path)
}
