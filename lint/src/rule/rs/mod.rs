use std::path::Path;

use crate::diagnostic::Diagnostic;
use crate::files::FileSet;

pub fn check(_path: &Path, _files: &FileSet) -> Vec<Diagnostic> {
    Vec::new()
}

pub fn check_folder(_path: &Path, _files: &FileSet) -> Vec<Diagnostic> {
    Vec::new()
}
