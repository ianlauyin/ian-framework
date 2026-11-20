pub(crate) mod env_name;
pub(crate) mod exception_code;
pub(crate) mod exception_new;
pub(crate) mod folder;
pub(crate) mod impl_order;
pub(crate) mod inherent_impl;
pub(crate) mod mod_file;
pub(crate) mod mutex_lock;
pub(crate) mod no_comments;
pub(crate) mod place;
pub(crate) mod private_static;
pub(crate) mod reexport_mod;
pub(crate) mod result_type;
pub(crate) mod source_order;
pub(crate) mod super_path;
pub(crate) mod test_attr;

use std::path::{Path, PathBuf};

use crate::config::Ruleset;
use crate::diagnostic::Diagnostic;
use crate::files::FileSet;
use crate::rule::ruleset::{RsRule, rules};

pub(crate) fn check(path: &Path, srcs: &[PathBuf], ruleset: Ruleset) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for rule in rules(ruleset).rs {
        let RsRule::File(check) = rule else {
            continue;
        };
        diagnostics.extend(check(path, srcs));
    }
    diagnostics
}

pub(crate) fn check_folder(
    path: &Path,
    files: &FileSet,
    srcs: &[PathBuf],
    ruleset: Ruleset,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for rule in rules(ruleset).rs {
        let RsRule::Folder(check) = rule else {
            continue;
        };
        diagnostics.extend(check(path, files, srcs));
    }
    diagnostics
}
