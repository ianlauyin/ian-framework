// The component folder has no `index.tsx`.
// Add an `index.tsx` that exports the component.
// Why a lint rule: Each component is a folder, so a file imports the component by the folder name.

use std::path::{Path, PathBuf};

use crate::config::Side;
use crate::files::{FileSet, without_dot};
use crate::rule::diagnostic::Diagnostic;
use crate::rule::folder::FolderRule;

const RULE: &str = "component-folder";

pub(crate) const FOLDER_RULE: FolderRule = FolderRule::new(Side::Fe, check);

pub fn check(path: &Path, files: &FileSet, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    let Some(parent) = path.parent().map(without_dot) else {
        return Vec::new();
    };
    if !srcs
        .iter()
        .any(|src| without_dot(src).join("component") == parent)
        || files.has_file(&path.join("index.tsx"))
    {
        return Vec::new();
    }
    vec![Diagnostic::new(
        path,
        1,
        1,
        RULE,
        "The component folder has no `index.tsx`.",
        "Add an `index.tsx` that exports the component.",
    )]
}
