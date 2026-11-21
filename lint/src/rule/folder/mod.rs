use std::path::{Path, PathBuf};

use crate::config::Side;
use crate::files::FileSet;
use crate::rule::diagnostic::Diagnostic;

pub(crate) mod fe_index_file;
pub(crate) mod fe_root_reexport;
pub(crate) mod fe_test_path;
pub(crate) mod rs_test_tree;

pub(crate) struct FolderRule {
    pub(crate) side: Side,
    pub(crate) check: fn(&Path, &FileSet, &[PathBuf]) -> Vec<Diagnostic>,
}

impl FolderRule {
    pub(crate) const fn new(
        side: Side,
        check: fn(&Path, &FileSet, &[PathBuf]) -> Vec<Diagnostic>,
    ) -> Self {
        Self { side, check }
    }
}
