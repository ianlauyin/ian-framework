use crate::rule::diagnostic::Diagnostic;
use crate::rule::edit::Edit;
use crate::rule::fe::css::file::CssFile;

pub(crate) struct CssRule {
    pub(crate) check: fn(&CssFile<'_>) -> Vec<Diagnostic>,
    pub(crate) edit: Option<fn(&CssFile<'_>) -> Vec<Edit>>,
}

impl CssRule {
    pub(crate) const fn new(
        check: fn(&CssFile<'_>) -> Vec<Diagnostic>,
        edit: Option<fn(&CssFile<'_>) -> Vec<Edit>>,
    ) -> Self {
        Self { check, edit }
    }
}
