use std::path::Path;

use crate::diagnostic::Diagnostic;

pub fn check(_path: &Path) -> Vec<Diagnostic> {
    Vec::new()
}
