use crate::rule::diagnostic::Diagnostic;
use crate::rule::fe::css::source::CssFile;

pub(crate) struct CssRule {
    pub(crate) check: fn(&CssFile<'_>) -> Vec<Diagnostic>,
}

impl CssRule {
    pub(crate) const fn new(check: fn(&CssFile<'_>) -> Vec<Diagnostic>) -> Self {
        Self { check }
    }
}
