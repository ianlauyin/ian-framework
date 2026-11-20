use std::path::{Path, PathBuf};

use crate::files::FileSet;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::edit::Edit;
use crate::rule::rs::file::env_name;
use crate::rule::rs::file::exception_code;
use crate::rule::rs::file::exception_new;
use crate::rule::rs::file::impl_order;
use crate::rule::rs::file::inherent_impl;
use crate::rule::rs::file::mod_file;
use crate::rule::rs::file::mutex_lock;
use crate::rule::rs::file::no_comments;
use crate::rule::rs::file::private_static;
use crate::rule::rs::file::reexport_mod;
use crate::rule::rs::file::result_type;
use crate::rule::rs::file::source_order;
use crate::rule::rs::file::super_path;
use crate::rule::rs::file::test_attr;
use crate::rule::rs::folder::test_tree;
use crate::rule::rs::source::RsFile;

pub(crate) struct RsFileRule {
    pub(crate) check: fn(&RsFile<'_>) -> Vec<Diagnostic>,
    pub(crate) edit: Option<fn(&RsFile<'_>) -> Vec<Edit>>,
}

pub(crate) struct RsFolderRule {
    pub(crate) check: fn(&Path, &FileSet, &[PathBuf]) -> Vec<Diagnostic>,
    pub(crate) edit: Option<fn(&Path, &FileSet, &[PathBuf]) -> Vec<Edit>>,
}

pub(crate) enum RsRule {
    File(RsFileRule),
    Folder(RsFolderRule),
}

impl RsRule {
    pub(crate) const fn file(
        check: fn(&RsFile<'_>) -> Vec<Diagnostic>,
        edit: Option<fn(&RsFile<'_>) -> Vec<Edit>>,
    ) -> Self {
        Self::File(RsFileRule { check, edit })
    }

    pub(crate) const fn folder(
        check: fn(&Path, &FileSet, &[PathBuf]) -> Vec<Diagnostic>,
        edit: Option<fn(&Path, &FileSet, &[PathBuf]) -> Vec<Edit>>,
    ) -> Self {
        Self::Folder(RsFolderRule { check, edit })
    }
}

pub(crate) const FRAMEWORK: &[RsRule] = &[
    RsRule::file(private_static::check, None),
    RsRule::file(no_comments::check, None),
    RsRule::file(super_path::check, None),
    RsRule::file(test_attr::check, None),
    RsRule::file(env_name::check, None),
    RsRule::file(exception_new::check, None),
    RsRule::file(exception_code::check, None),
    RsRule::file(mutex_lock::check, None),
    RsRule::file(source_order::check, Some(source_order::edit)),
    RsRule::file(impl_order::check, Some(impl_order::edit)),
    RsRule::file(mod_file::check, None),
    RsRule::file(inherent_impl::check, None),
    RsRule::file(reexport_mod::check, None),
    RsRule::file(result_type::check, None),
    RsRule::folder(test_tree::check, None),
];

pub(crate) const PROJECT: &[RsRule] = &[
    RsRule::file(private_static::check, None),
    RsRule::file(no_comments::check, None),
    RsRule::file(super_path::check, None),
    RsRule::file(test_attr::check, None),
    RsRule::file(exception_new::check, None),
    RsRule::file(exception_code::check, None),
    RsRule::file(mutex_lock::check, None),
    RsRule::file(impl_order::check, Some(impl_order::edit)),
    RsRule::file(inherent_impl::check, None),
    RsRule::file(reexport_mod::check, None),
    RsRule::file(result_type::check, None),
    RsRule::folder(test_tree::check, None),
];
