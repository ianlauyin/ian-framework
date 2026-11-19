use std::path::{Path, PathBuf};

use crate::config::Ruleset;
use crate::diagnostic::Diagnostic;
use crate::files::FileSet;
use crate::rule::fe::css::{empty_line, nested};
use crate::rule::fe::folder::{index, reexport, test};
use crate::rule::fe::ts::{comments, declaration, default_export, restricted_imports};
use crate::rule::rs::folder::check as check_test_tree;
use crate::rule::rs::{
    check_comments, check_exception_code, check_impl_order, check_inherent_impl,
    check_private_static, check_reexport_mod, check_result_type, check_source_order,
    check_super_path, env_name, exception_new, mod_file, mutex_lock, test_attr,
};

pub(crate) struct Rules {
    pub(crate) css: &'static [fn(&Path) -> Vec<Diagnostic>],
    pub(crate) ts: &'static [TsRule],
    pub(crate) rs: &'static [RsRule],
}

pub(crate) enum TsRule {
    File(fn(&Path) -> Vec<Diagnostic>),
    Folder(fn(&Path, &FileSet, &[PathBuf]) -> Vec<Diagnostic>),
}

pub(crate) enum RsRule {
    File(fn(&Path, &[PathBuf]) -> Vec<Diagnostic>),
    Folder(fn(&Path, &FileSet, &[PathBuf]) -> Vec<Diagnostic>),
}

const FRAMEWORK: Rules = Rules {
    css: &[empty_line, nested],
    ts: &[
        TsRule::File(restricted_imports),
        TsRule::File(comments),
        TsRule::File(default_export),
        TsRule::File(declaration),
        TsRule::Folder(index),
        TsRule::Folder(reexport),
        TsRule::Folder(test),
    ],
    rs: &[
        RsRule::File(check_private_static),
        RsRule::File(check_comments),
        RsRule::File(check_super_path),
        RsRule::File(test_attr::check),
        RsRule::File(env_name::check),
        RsRule::File(exception_new::check),
        RsRule::File(check_exception_code),
        RsRule::File(mutex_lock::check),
        RsRule::File(check_source_order),
        RsRule::File(check_impl_order),
        RsRule::File(mod_file::check),
        RsRule::File(check_inherent_impl),
        RsRule::File(check_reexport_mod),
        RsRule::File(check_result_type),
        RsRule::Folder(check_test_tree),
    ],
};

const PROJECT: Rules = Rules {
    css: &[empty_line, nested],
    ts: &[
        TsRule::File(restricted_imports),
        TsRule::File(comments),
        TsRule::File(declaration),
        TsRule::Folder(test),
    ],
    rs: &[
        RsRule::File(check_private_static),
        RsRule::File(check_comments),
        RsRule::File(check_super_path),
        RsRule::File(test_attr::check),
        RsRule::File(exception_new::check),
        RsRule::File(check_exception_code),
        RsRule::File(mutex_lock::check),
        RsRule::File(check_impl_order),
        RsRule::File(check_inherent_impl),
        RsRule::File(check_reexport_mod),
        RsRule::File(check_result_type),
        RsRule::Folder(check_test_tree),
    ],
};

pub(crate) fn rules(ruleset: Ruleset) -> &'static Rules {
    match ruleset {
        Ruleset::Framework => &FRAMEWORK,
        Ruleset::Project => &PROJECT,
    }
}
