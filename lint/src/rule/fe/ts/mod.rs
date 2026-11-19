pub(crate) mod declaration_order;
pub(crate) mod no_comments;
pub(crate) mod no_default_export;
pub(crate) mod no_restricted_imports;

use std::path::Path;

use crate::diagnostic::Diagnostic;

pub(crate) fn restricted_imports(path: &Path) -> Vec<Diagnostic> {
    no_restricted_imports::check(path)
}

pub(crate) fn comments(path: &Path) -> Vec<Diagnostic> {
    no_comments::check(path)
}

pub(crate) fn default_export(path: &Path) -> Vec<Diagnostic> {
    no_default_export::check(path)
}

pub(crate) fn declaration(path: &Path) -> Vec<Diagnostic> {
    declaration_order::check(path)
}
