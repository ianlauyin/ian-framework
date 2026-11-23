use std::path::{Path, PathBuf};

use crate::config::Side;
use crate::files::FileSet;
use crate::rule::diagnostic::Diagnostic;

pub mod fe_ajax_tree;
pub mod fe_component_folder;
pub mod fe_index_file;
pub mod fe_module_folder;
pub mod fe_root_reexport;
pub mod fe_test_tree;
pub mod rs_router_folder;
pub mod rs_test_tree;

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
