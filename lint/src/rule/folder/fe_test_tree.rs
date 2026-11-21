// The test file has no source file `{path}`.
// Give the test the same path as the source file.
// Why a lint rule: The test tree matches the source tree.

use std::path::{Path, PathBuf};

use crate::config::Side;
use crate::files::{FileSet, file_name, without_dot};
use crate::rule::diagnostic::Diagnostic;
use crate::rule::folder::FolderRule;

const RULE: &str = "test-tree";

pub(crate) const FOLDER_RULE: FolderRule = FolderRule::new(Side::Fe, check);

pub fn check(path: &Path, files: &FileSet, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    if !srcs.iter().any(|src| without_dot(src) == without_dot(path)) {
        return Vec::new();
    }
    let Some(test_root) = path.parent().map(|parent| parent.join("tests")) else {
        return Vec::new();
    };
    let mut diagnostics = Vec::new();
    for file in files.files() {
        let Some(source) = mirrored_source(path, &test_root, file) else {
            continue;
        };
        if files.has_file(&source) {
            continue;
        }
        diagnostics.push(Diagnostic::new(
            file,
            1,
            1,
            RULE,
            format!("The test file has no source file `{}`.", source.display()),
            "Give the test the same path as the source file.",
        ));
    }
    diagnostics
}

fn mirrored_source(src: &Path, test_root: &Path, test_file: &Path) -> Option<PathBuf> {
    let relative = test_file.strip_prefix(test_root).ok()?;
    let name = file_name(test_file);
    let source_name = if let Some(stem) = name.strip_suffix(".test.tsx") {
        format!("{stem}.tsx")
    } else if let Some(stem) = name.strip_suffix(".test.ts") {
        format!("{stem}.ts")
    } else {
        return None;
    };
    let mut source = src.join(relative);
    source.set_file_name(source_name);
    Some(source)
}
