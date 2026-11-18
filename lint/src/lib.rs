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
            tool: Tool::IanLint,
            output: walk_errors.join("\n"),
        });
    }
    report.diagnostics.sort();
    report
}

/// Sends each file to the rule set for its kind.
pub fn check_files(files: &FileSet) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for path in files.files() {
        if files::is_frontend_source(path) {
            diagnostics.extend(rule::fe::check(path, files));
        } else if files::is_rust_source(path) {
            diagnostics.extend(rule::rs::check(path, files));
        }
    }
    diagnostics
}
