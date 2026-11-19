pub(crate) mod declaration_order;
pub(crate) mod no_comments;
pub(crate) mod no_default_export;
pub(crate) mod no_restricted_imports;

use std::path::Path;

use crate::diagnostic::Diagnostic;

pub(crate) fn no_restricted_imports(path: &Path) -> Vec<Diagnostic> {
    no_restricted_imports::check(path)
}

pub(crate) fn no_comments(path: &Path) -> Vec<Diagnostic> {
    no_comments::check(path)
}

pub(crate) fn no_default_export(path: &Path) -> Vec<Diagnostic> {
    no_default_export::check(path)
}

pub(crate) fn declaration_order(path: &Path) -> Vec<Diagnostic> {
    declaration_order::check(path)
}
