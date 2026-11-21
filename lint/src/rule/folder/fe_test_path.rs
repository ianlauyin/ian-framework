// A test file is in the source folder.
// Move the test to the `tests` folder.
// Why a lint rule: A test stays out of the source folder.

use std::path::{Path, PathBuf};

use crate::config::Side;
use crate::files::FileSet;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::folder::FolderRule;

const RULE: &str = "test-path";

pub(crate) const FOLDER_RULE: FolderRule = FolderRule::new(Side::Fe, check);

pub fn check(path: &Path, files: &FileSet, _srcs: &[PathBuf]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for file in files.files() {
        if file.parent() != Some(path) {
            continue;
        }
        if !is_test_name(crate::files::file_name(file)) {
            continue;
        }
        diagnostics.push(Diagnostic::new(
            file,
            1,
            1,
            RULE,
            "A test file is in the source folder.",
            "Move the test to the `tests` folder.",
        ));
    }
    diagnostics
}

fn is_test_name(name: &str) -> bool {
    name.ends_with(".test.ts") || name.ends_with(".test.tsx")
}
