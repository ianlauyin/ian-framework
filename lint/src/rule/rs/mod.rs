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

use crate::diagnostic::Diagnostic;
use crate::files::FileSet;

pub fn check(path: &Path, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    let mut diagnostics = private_static::check(path);
    diagnostics.extend(no_comments::check(path));
    diagnostics.extend(super_path::check(path));
    diagnostics.extend(test_attr::check(path, srcs));
    diagnostics.extend(env_name::check(path, srcs));
    diagnostics.extend(exception_new::check(path, srcs));
    diagnostics.extend(exception_code::check(path));
    diagnostics.extend(mutex_lock::check(path, srcs));
    diagnostics.extend(source_order::check(path));
    diagnostics.extend(impl_order::check(path));
    diagnostics.extend(mod_file::check(path, srcs));
    diagnostics.extend(inherent_impl::check(path));
    diagnostics.extend(reexport_mod::check(path));
    diagnostics.extend(result_type::check(path));
    diagnostics
}

pub fn check_folder(path: &Path, files: &FileSet, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    folder::check(path, files, srcs)
}
