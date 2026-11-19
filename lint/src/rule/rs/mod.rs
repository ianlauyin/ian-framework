mod folder;
pub(crate) mod private_static;

use std::path::Path;

use crate::diagnostic::Diagnostic;
use crate::files::FileSet;

pub fn check(path: &Path) -> Vec<Diagnostic> {
    private_static::check(path)
}

pub fn check_folder(path: &Path, files: &FileSet) -> Vec<Diagnostic> {
    folder::check(path, files)
}
