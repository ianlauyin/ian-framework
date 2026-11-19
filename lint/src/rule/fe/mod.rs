pub(crate) mod css;
pub(crate) mod folder;
pub(crate) mod ts;

use std::path::{Path, PathBuf};

use crate::config::Ruleset;
use crate::diagnostic::Diagnostic;
use crate::files::FileSet;
use crate::rule::ruleset::{TsRule, rules};

pub(crate) fn check(path: &Path, ruleset: Ruleset) -> Vec<Diagnostic> {
    let rules = rules(ruleset);
    if crate::files::file_name(path).ends_with(".css") {
        let mut diagnostics = Vec::new();
        for check in rules.css {
            diagnostics.extend(check(path));
        }
        diagnostics
    } else {
        let mut diagnostics = Vec::new();
        for rule in rules.ts {
            let TsRule::File(check) = rule else {
                continue;
            };
            diagnostics.extend(check(path));
        }
        diagnostics
    }
}

pub(crate) fn check_folder(
    path: &Path,
    files: &FileSet,
    srcs: &[PathBuf],
    ruleset: Ruleset,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for rule in rules(ruleset).ts {
        let TsRule::Folder(check) = rule else {
            continue;
        };
        diagnostics.extend(check(path, files, srcs));
    }
    diagnostics
}
