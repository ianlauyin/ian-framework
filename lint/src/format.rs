use std::path::Path;

use crate::config::{Config, Side};
use crate::files::FileSet;
use crate::{Failure, Report};

pub(crate) fn format(root: &Path, side: Side) -> Report {
    let (config, files, mut failures) = match crate::check::start(root) {
        Ok(start) => start,
        Err(report) => return report,
    };
    failures.extend(write_edits(&files, &config, side));
    crate::check::finish(&files, &config, side, failures)
}

fn write_edits(files: &FileSet, config: &Config, side: Side) -> Vec<Failure> {
    let mut failures = Vec::new();
    for path in files.files() {
        if side.checks_fe() && crate::files::is_frontend_source(path, &config.fe) {
            failures.extend(crate::rule::fe::run::rewrite(path, config.ruleset));
        } else if side.checks_rs() && crate::files::is_rust_source(path, &config.be) {
            failures.extend(crate::rule::rs::run::rewrite(path, config.ruleset));
        }
    }
    for path in files.dirs() {
        if side.checks_fe() && crate::files::in_src(path, &config.fe) {
            failures.extend(crate::rule::fe::run::rewrite_folder(
                path,
                files,
                &config.fe,
                config.ruleset,
            ));
        }
        if side.checks_rs() && crate::files::in_src(path, &config.be) {
            failures.extend(crate::rule::rs::run::rewrite_folder(
                path,
                files,
                &config.be,
                config.ruleset,
            ));
        }
    }
    failures
}
