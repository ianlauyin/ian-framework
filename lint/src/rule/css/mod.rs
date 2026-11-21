use crate::file::css::source::CssFile;
use crate::rule::diagnostic::Diagnostic;

pub mod empty_line_before;
pub mod selector_nested_pattern;

pub(crate) struct CssRule {
    pub(crate) check: fn(&CssFile<'_>) -> Vec<Diagnostic>,
}

impl CssRule {
    pub(crate) const fn new(check: fn(&CssFile<'_>) -> Vec<Diagnostic>) -> Self {
        Self { check }
    }
}
