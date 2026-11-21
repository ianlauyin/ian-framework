use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;

pub(crate) mod declaration_order;
pub(crate) mod no_comments;
pub(crate) mod no_default_export;
pub(crate) mod no_restricted_imports;

pub(crate) struct TsRule {
    pub(crate) check: fn(&TsFile<'_>) -> Vec<Diagnostic>,
}

impl TsRule {
    pub(crate) const fn file(check: fn(&TsFile<'_>) -> Vec<Diagnostic>) -> Self {
        Self { check }
    }
}
