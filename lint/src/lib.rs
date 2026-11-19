use std::path::Path;

use config::Config;
use diagnostic::Diagnostic;
use files::FileSet;

pub mod config;
pub mod diagnostic;
pub mod files;
pub mod rule;

pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
    pub failures: Vec<Failure>,
}

#[derive(Debug)]
pub struct Failure {
    pub output: String,
}

pub fn lint(root: &Path) -> Report {
    let mut failures = Vec::new();
    let config = match config::load(root) {
        Ok(config) => Some(config),
        Err(output) => {
            failures.push(Failure { output });
            None
        }
    };
    let (files, walk_errors) = FileSet::walk(root);
    if !walk_errors.is_empty() {
        failures.push(Failure {
            output: walk_errors.join("\n"),
        });
    }
    let mut diagnostics = Vec::new();
    if let Some(config) = &config {
        diagnostics = check_files(&files, config);
        diagnostics.sort();
    }
    Report {
        diagnostics,
        failures,
    }
}

pub fn check_files(files: &FileSet, config: &Config) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for path in files.files() {
        if files::is_frontend_source(path, &config.fe) {
            diagnostics.extend(rule::fe::check(path, files));
        } else if files::is_rust_source(path, &config.be) {
            diagnostics.extend(rule::rs::check(path, files));
        }
    }
    diagnostics
}
