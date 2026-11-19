pub(crate) mod no_restricted_imports;

use std::path::Path;

use crate::diagnostic::Diagnostic;

pub fn check(path: &Path) -> Vec<Diagnostic> {
    no_restricted_imports::check(path)
}
