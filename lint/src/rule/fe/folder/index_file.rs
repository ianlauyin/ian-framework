// The folder has no `index.ts`.
// Add an `index.ts`.
// Why a lint rule: A folder has an `index.ts` so a project can import the folder.

use std::path::Path;

use crate::diagnostic::Diagnostic;
use crate::files::FileSet;

const RULE: &str = "folder-index";

pub fn check(path: &Path, files: &FileSet) -> Vec<Diagnostic> {
    if files.has_file(&path.join("index.ts")) {
        return Vec::new();
    }
    vec![Diagnostic::new(
        path,
        1,
        1,
        RULE,
        "The folder has no `index.ts`.",
        "Add an `index.ts`.",
    )]
}
