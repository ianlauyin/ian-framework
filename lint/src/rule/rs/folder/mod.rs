pub(crate) mod test_tree;

use std::path::{Path, PathBuf};

use crate::diagnostic::Diagnostic;
use crate::files::FileSet;

pub(crate) fn check(path: &Path, files: &FileSet, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    test_tree::check(path, files, srcs)
}
