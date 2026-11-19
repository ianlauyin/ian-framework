pub(crate) mod index_file;
pub(crate) mod root_reexport;
pub(crate) mod test_path;

use std::path::{Path, PathBuf};

use crate::diagnostic::Diagnostic;
use crate::files::FileSet;

pub(crate) fn index_file(path: &Path, files: &FileSet, _: &[PathBuf]) -> Vec<Diagnostic> {
    index_file::check(path, files)
}

pub(crate) fn root_reexport(path: &Path, files: &FileSet, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    root_reexport::check(path, files, srcs)
}

pub(crate) fn test_path(path: &Path, files: &FileSet, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    test_path::check(path, files, srcs)
}
