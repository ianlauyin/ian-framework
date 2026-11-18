pub mod diagnostic;
pub mod files;
pub mod rule;

use std::path::Path;

use diagnostic::{Diagnostic, Tool};
use files::FileSet;

/// The result of one lint run.
pub struct Report {
    /// The diagnostics, in the order of path, then line.
    pub diagnostics: Vec<Diagnostic>,
    /// The parts of the run that did not finish. A failure is not a lint error.
    pub failures: Vec<Failure>,
}

/// A part of the run that did not finish.
#[derive(Debug)]
pub struct Failure {
    pub tool: Tool,
    /// The reason that the part did not finish.
    pub output: String,
}

/// Lints all files below `root`.
pub fn lint(root: &Path) -> Report {
    let (files, walk_errors) = FileSet::walk(root);
    let mut report = Report {
        diagnostics: check_files(&files),
        failures: Vec::new(),
    };
    if !walk_errors.is_empty() {
        report.failures.push(Failure {
            tool: Tool::CodeStyle,
            output: walk_errors.join("\n"),
        });
    }
    report.diagnostics.sort();
    report
}

/// The rules of this program for the files in `files`.
#[allow(clippy::missing_const_for_fn, reason = "A rule that is added later reads the files.")]
pub fn check_files(files: &FileSet) -> Vec<Diagnostic> {
    let _: &FileSet = files;
    Vec::new()
}
