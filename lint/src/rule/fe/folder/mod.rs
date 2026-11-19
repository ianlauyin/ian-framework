pub(crate) mod index_file;
pub(crate) mod root_reexport;
pub(crate) mod test_path;

use std::path::{Path, PathBuf};

use crate::diagnostic::Diagnostic;
use crate::files::FileSet;

pub fn check(path: &Path, files: &FileSet, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    let mut diagnostics = index_file::check(path, files);
    diagnostics.extend(root_reexport::check(path, files, srcs));
    diagnostics.extend(test_path::check(path, files, srcs));
    diagnostics
}
