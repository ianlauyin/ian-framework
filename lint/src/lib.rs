#[cfg(test)]
mod test;

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
    let config = match config::load(root) {
        Ok(config) => config,
        Err(output) => {
            return Report {
                diagnostics: Vec::new(),
                failures: vec![Failure { output }],
            };
        }
    };
    let (files, walk_errors) = FileSet::walk(root);
    let mut failures = Vec::new();
    if !walk_errors.is_empty() {
        failures.push(Failure {
            output: walk_errors.join("\n"),
        });
    }
    let mut diagnostics = check_files(&files, &config);
    diagnostics.extend(check_folders(&files, &config));
    diagnostics.sort();
    Report {
        diagnostics,
        failures,
    }
}

pub fn check_files(files: &FileSet, config: &Config) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for path in files.files() {
        if files::is_frontend_source(path, &config.fe) || files::is_style_file(path, &config.fe) {
            diagnostics.extend(rule::fe::check(path, files));
        } else if files::is_rust_source(path, &config.be) {
            diagnostics.extend(rule::rs::check(path, files));
        }
    }
    diagnostics
}

pub fn check_folders(files: &FileSet, config: &Config) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for path in files.dirs() {
        if files::in_src(path, &config.fe) {
            diagnostics.extend(rule::fe::check_folder(path, files));
        }
        if files::in_src(path, &config.be) {
            diagnostics.extend(rule::rs::check_folder(path, files));
        }
    }
    diagnostics
}
