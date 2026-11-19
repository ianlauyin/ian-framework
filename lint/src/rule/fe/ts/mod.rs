pub(crate) mod no_comments;
pub(crate) mod no_restricted_imports;

use std::path::Path;

use crate::diagnostic::Diagnostic;

pub fn check(path: &Path) -> Vec<Diagnostic> {
    let mut diagnostics = no_restricted_imports::check(path);
    diagnostics.extend(no_comments::check(path));
    diagnostics
}
