// The folder has no `index.ts` or `index.tsx`.
// Add an `index.ts`.
// Why a lint rule: A folder has an `index.ts` so a project can import the folder.

use std::path::{Path, PathBuf};

use crate::config::Side;
use crate::files::FileSet;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::folder::FolderRule;

const RULE: &str = "folder-index";

pub(crate) const FOLDER_RULE: FolderRule = FolderRule::new(Side::Fe, check);

pub fn check(path: &Path, files: &FileSet, _srcs: &[PathBuf]) -> Vec<Diagnostic> {
    if ["index.ts", "index.tsx"]
        .iter()
        .any(|name| files.has_file(&path.join(name)))
    {
        return Vec::new();
    }
    vec![Diagnostic::new(
        path,
        1,
        1,
        RULE,
        "The folder has no `index.ts` or `index.tsx`.",
        "Add an `index.ts`.",
    )]
}
