use std::path::Path;

use crate::config::{Config, Side};
use crate::files::FileSet;
use crate::{Failure, Report};

pub fn format(root: &Path, side: Side) -> Report {
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
    failures.extend(write_edits(&files, &config, side));
    let mut diagnostics = crate::check::check_files(&files, &config, side);
    diagnostics.extend(crate::check::check_folders(&files, &config, side));
    diagnostics.sort();
    Report {
        diagnostics,
        failures,
    }
}

fn write_edits(files: &FileSet, config: &Config, side: Side) -> Vec<Failure> {
    let mut failures = Vec::new();
    for path in files.files() {
        if side.checks_fe() && crate::files::is_frontend_source(path, &config.fe) {
            failures.extend(crate::rule::fe::rewrite(path, config.ruleset));
        } else if side.checks_rs() && crate::files::is_rust_source(path, &config.be) {
            failures.extend(crate::rule::rs::rewrite(path, config.ruleset));
        }
    }
    for path in files.dirs() {
        if side.checks_fe() && crate::files::in_src(path, &config.fe) {
            failures.extend(crate::rule::fe::rewrite_folder(
                path,
                files,
                &config.fe,
                config.ruleset,
            ));
        }
        if side.checks_rs() && crate::files::in_src(path, &config.be) {
            failures.extend(crate::rule::rs::rewrite_folder(
                path,
                files,
                &config.be,
                config.ruleset,
            ));
        }
    }
    failures
}
