use std::path::{Path, PathBuf};

use crate::config::Ruleset;
use crate::diagnostic::Diagnostic;
use crate::files::FileSet;
use crate::rule::fe::css;
use crate::rule::fe::folder as fe_folder;
use crate::rule::fe::ts;
use crate::rule::rs;
use crate::rule::rs::folder as rs_folder;

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
    css: &[css::rule_empty_line_before, css::selector_nested_pattern],
    ts: &[
        TsRule::File(ts::no_restricted_imports),
        TsRule::File(ts::no_comments),
        TsRule::File(ts::no_default_export),
        TsRule::File(ts::declaration_order),
        TsRule::Folder(fe_folder::index_file),
        TsRule::Folder(fe_folder::root_reexport),
        TsRule::Folder(fe_folder::test_path),
    ],
    rs: &[
        RsRule::File(rs::private_static),
        RsRule::File(rs::no_comments),
        RsRule::File(rs::super_path),
        RsRule::File(rs::test_attr::check),
        RsRule::File(rs::env_name::check),
        RsRule::File(rs::exception_new::check),
        RsRule::File(rs::exception_code),
        RsRule::File(rs::mutex_lock::check),
        RsRule::File(rs::source_order),
        RsRule::File(rs::impl_order),
        RsRule::File(rs::mod_file::check),
        RsRule::File(rs::inherent_impl),
        RsRule::File(rs::reexport_mod),
        RsRule::File(rs::result_type),
        RsRule::Folder(rs_folder::test_tree),
    ],
};

const PROJECT: Rules = Rules {
    css: &[css::rule_empty_line_before, css::selector_nested_pattern],
    ts: &[
        TsRule::File(ts::no_restricted_imports),
        TsRule::File(ts::no_comments),
        TsRule::File(ts::declaration_order),
        TsRule::Folder(fe_folder::test_path),
    ],
    rs: &[
        RsRule::File(rs::private_static),
        RsRule::File(rs::no_comments),
        RsRule::File(rs::super_path),
        RsRule::File(rs::test_attr::check),
        RsRule::File(rs::exception_new::check),
        RsRule::File(rs::exception_code),
        RsRule::File(rs::mutex_lock::check),
        RsRule::File(rs::impl_order),
        RsRule::File(rs::inherent_impl),
        RsRule::File(rs::reexport_mod),
        RsRule::File(rs::result_type),
        RsRule::Folder(rs_folder::test_tree),
    ],
};

pub(crate) fn rules(ruleset: Ruleset) -> &'static Rules {
    match ruleset {
        Ruleset::Framework => &FRAMEWORK,
        Ruleset::Project => &PROJECT,
    }
}
