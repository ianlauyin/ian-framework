mod css;
mod folder;
mod ts;

use std::path::Path;

use crate::diagnostic::Diagnostic;
use crate::files::FileSet;

pub fn check(path: &Path, files: &FileSet) -> Vec<Diagnostic> {
    if crate::files::file_name(path).ends_with(".css") {
        css::check(path, files)
    } else {
        ts::check(path, files)
    }
}

pub fn check_folder(path: &Path, files: &FileSet) -> Vec<Diagnostic> {
    folder::check(path, files)
}
