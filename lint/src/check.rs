use std::path::Path;

use crate::config::{Config, Side};
use crate::files::FileSet;
use crate::rule::diagnostic::Diagnostic;
use crate::{Failure, Report};

pub(crate) fn check(root: &Path, side: Side) -> Report {
    let config = match crate::config::load(root) {
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

fn check_files(files: &FileSet, config: &Config, side: Side) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for path in files.files() {
        if side.checks_fe() && crate::files::is_frontend_source(path, &config.fe) {
            diagnostics.extend(crate::rule::fe::run::check(path, config.ruleset));
        } else if side.checks_rs() && crate::files::is_rust_source(path, &config.be) {
            diagnostics.extend(crate::rule::rs::run::check(
                path,
                &config.be,
                config.ruleset,
            ));
        }
    }
    diagnostics
}

fn check_folders(files: &FileSet, config: &Config, side: Side) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for path in files.dirs() {
        if side.checks_fe() && crate::files::in_src(path, &config.fe) {
            diagnostics.extend(crate::rule::fe::run::check_folder(
                path,
                files,
                &config.fe,
                config.ruleset,
            ));
        }
        if side.checks_rs() && crate::files::in_src(path, &config.be) {
            diagnostics.extend(crate::rule::rs::run::check_folder(
                path,
                files,
                &config.be,
                config.ruleset,
            ));
        }
    }
    diagnostics
}
