// The folder has no `index.ts`.
// Add an `index.ts`.
// Why a lint rule: A folder has an `index.ts` so a project can import the folder.

use std::path::{Path, PathBuf};

use crate::files::FileSet;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::fe::ts::rule::TsRule;

const RULE: &str = "folder-index";

pub(crate) const TS_RULE: TsRule = TsRule::folder(check);

pub(crate) fn check(path: &Path, files: &FileSet, _srcs: &[PathBuf]) -> Vec<Diagnostic> {
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
