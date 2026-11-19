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

pub(crate) fn check_private_static(path: &Path, _: &[PathBuf]) -> Vec<Diagnostic> {
    private_static::check(path)
}

pub(crate) fn check_comments(path: &Path, _: &[PathBuf]) -> Vec<Diagnostic> {
    no_comments::check(path)
}

pub(crate) fn check_super_path(path: &Path, _: &[PathBuf]) -> Vec<Diagnostic> {
    super_path::check(path)
}

pub(crate) fn check_exception_code(path: &Path, _: &[PathBuf]) -> Vec<Diagnostic> {
    exception_code::check(path)
}

pub(crate) fn check_source_order(path: &Path, _: &[PathBuf]) -> Vec<Diagnostic> {
    source_order::check(path)
}

pub(crate) fn check_impl_order(path: &Path, _: &[PathBuf]) -> Vec<Diagnostic> {
    impl_order::check(path)
}

pub(crate) fn check_inherent_impl(path: &Path, _: &[PathBuf]) -> Vec<Diagnostic> {
    inherent_impl::check(path)
}

pub(crate) fn check_reexport_mod(path: &Path, _: &[PathBuf]) -> Vec<Diagnostic> {
    reexport_mod::check(path)
}

pub(crate) fn check_result_type(path: &Path, _: &[PathBuf]) -> Vec<Diagnostic> {
    result_type::check(path)
}
