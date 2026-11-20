// A test file is in the source folder.
// Move the test to the test folder.
// The test file has no source file `{path}`.
// Give the test the same path as the source file.
// Why a lint rule: A test stays out of the source folder. The test path matches the source path.

use std::path::{Path, PathBuf};

use crate::files::FileSet;
use crate::rule::diagnostic::Diagnostic;

const RULE: &str = "test-path";

pub fn check(path: &Path, files: &FileSet, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    let mut diagnostics = source_tests(path, files);
    diagnostics.extend(mirrored_tests(path, files, srcs));
    diagnostics
}

fn source_tests(path: &Path, files: &FileSet) -> Vec<Diagnostic> {
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
            "Move the test to the test folder.",
        ));
    }
    diagnostics
}

fn mirrored_tests(path: &Path, files: &FileSet, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    if !srcs.iter().any(|src| src == path) {
        return Vec::new();
    }
    let Some(test_root) = path.parent().map(|parent| parent.join("test")) else {
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

fn is_test_name(name: &str) -> bool {
    name.ends_with(".test.ts") || name.ends_with(".test.tsx")
}

fn mirrored_source(src: &Path, test_root: &Path, test_file: &Path) -> Option<PathBuf> {
    let relative = test_file.strip_prefix(test_root).ok()?;
    let name = relative.file_name()?.to_str()?;
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
