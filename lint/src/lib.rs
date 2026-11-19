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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Side {
    All,
    Fe,
    Rs,
}

impl Side {
    fn checks_fe(self) -> bool {
        matches!(self, Self::All | Self::Fe)
    }

    fn checks_rs(self) -> bool {
        matches!(self, Self::All | Self::Rs)
    }
}

pub fn lint(root: &Path, side: Side) -> Report {
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
    let mut diagnostics = check_files(&files, &config, side);
    diagnostics.extend(check_folders(&files, &config, side));
    diagnostics.sort();
    Report {
        diagnostics,
        failures,
    }
}

pub fn check_files(files: &FileSet, config: &Config, side: Side) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for path in files.files() {
        if side.checks_fe() && files::is_frontend_source(path, &config.fe) {
            diagnostics.extend(rule::fe::check(path));
        } else if side.checks_rs() && files::is_rust_source(path, &config.be) {
            diagnostics.extend(rule::rs::check(path, &config.be));
        }
    }
    diagnostics
}

pub fn check_folders(files: &FileSet, config: &Config, side: Side) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for path in files.dirs() {
        if side.checks_fe() && files::in_src(path, &config.fe) {
            diagnostics.extend(rule::fe::check_folder(path, files, &config.fe));
        }
        if side.checks_rs() && files::in_src(path, &config.be) {
            diagnostics.extend(rule::rs::check_folder(path, files, &config.be));
        }
    }
    diagnostics
}
