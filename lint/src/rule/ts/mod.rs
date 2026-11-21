use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;

pub mod declaration_order;
pub mod no_comments;
pub mod no_default_export;
pub mod no_restricted_imports;
pub mod test_path;

pub(crate) struct TsRule {
    pub(crate) check: fn(&TsFile<'_>) -> Vec<Diagnostic>,
    pub(crate) tests: bool,
}

impl TsRule {
    pub(crate) const fn file(check: fn(&TsFile<'_>) -> Vec<Diagnostic>) -> Self {
        Self { check, tests: true }
    }

    pub(crate) const fn source(check: fn(&TsFile<'_>) -> Vec<Diagnostic>) -> Self {
        Self {
            check,
            tests: false,
        }
    }
}
