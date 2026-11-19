use std::path::Path;

use diagnostic::{Diagnostic, Tool};
use files::FileSet;

pub mod diagnostic;
pub mod files;
pub mod rule;

pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
    pub failures: Vec<Failure>,
}

#[derive(Debug)]
pub struct Failure {
    pub tool: Tool,
    pub output: String,
}

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
