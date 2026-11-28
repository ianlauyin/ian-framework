// The module folder has no `{name}`.
// Add `index.ts` for the code of the module and `types.ts` for its types.
// The `module` folder has no `index.ts`.
// Add `index.ts` for the module registration.
// Why a lint rule: Each module is a folder, so a file imports the module by the folder name and its types from `types.ts`.

use std::path::{Path, PathBuf};

use crate::config::Side;
use crate::files::{FileSet, without_dot};
use crate::rule::diagnostic::Diagnostic;
use crate::rule::folder::FolderRule;

const RULE: &str = "module-shape";

const FILES: [&str; 2] = ["index.ts", "types.ts"];

pub(crate) const FOLDER_RULE: FolderRule = FolderRule::new(Side::Fe, check);

pub fn check(path: &Path, files: &FileSet, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    let path = without_dot(path);
    if srcs
        .iter()
        .any(|src| without_dot(src).join("module") == path)
    {
        return module_index(&path, files).into_iter().collect();
    }
    let Some(parent) = path.parent().map(without_dot) else {
        return Vec::new();
    };
    if !srcs
        .iter()
        .any(|src| without_dot(src).join("module") == parent)
    {
        return Vec::new();
    }
    FILES
        .iter()
        .filter(|name| !files.has_file(&path.join(name)))
        .map(|name| {
            Diagnostic::new(
                &path,
                1,
                1,
                RULE,
                format!("The module folder has no `{name}`."),
                "Add `index.ts` for the code of the module and `types.ts` for its types.",
            )
        })
        .collect()
}

fn module_index(path: &Path, files: &FileSet) -> Option<Diagnostic> {
    if files.has_file(&path.join("index.ts")) {
        return None;
    }
    Some(Diagnostic::new(
        path,
        1,
        1,
        RULE,
        "The `module` folder has no `index.ts`.",
        "Add `index.ts` for the module registration.",
    ))
}
