mod css;
pub(crate) mod folder;
pub(crate) mod ts;

use std::path::{Path, PathBuf};

use crate::diagnostic::Diagnostic;
use crate::files::FileSet;

pub fn check(path: &Path) -> Vec<Diagnostic> {
    if crate::files::file_name(path).ends_with(".css") {
        css::check(path)
    } else {
        ts::check(path)
    }
}

pub fn check_folder(path: &Path, files: &FileSet, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    folder::check(path, files, srcs)
}
