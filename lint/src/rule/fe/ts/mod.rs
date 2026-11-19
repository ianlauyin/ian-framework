pub(crate) mod declaration_order;
pub(crate) mod no_comments;
pub(crate) mod no_default_export;
pub(crate) mod no_restricted_imports;

use std::path::Path;

use crate::diagnostic::Diagnostic;

pub fn check(path: &Path) -> Vec<Diagnostic> {
    let mut diagnostics = no_restricted_imports::check(path);
    diagnostics.extend(no_comments::check(path));
    diagnostics.extend(no_default_export::check(path));
    diagnostics.extend(declaration_order::check(path));
    diagnostics
}
