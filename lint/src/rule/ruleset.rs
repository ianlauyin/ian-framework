use std::path::{Path, PathBuf};

use crate::config::Ruleset;
use crate::diagnostic::Diagnostic;
use crate::files::FileSet;
use crate::rule::fe;
use crate::rule::fe::css::file::CssFile;
use crate::rule::fe::ts::file::TsFile;
use crate::rule::rs;
use crate::rule::rs::file::RsFile;

pub(crate) struct Rules {
    pub(crate) css: &'static [fn(&CssFile<'_>) -> Vec<Diagnostic>],
    pub(crate) ts: &'static [TsRule],
    pub(crate) rs: &'static [RsRule],
}

pub(crate) enum TsRule {
    File(fn(&TsFile<'_>) -> Vec<Diagnostic>),
    Folder(fn(&Path, &FileSet, &[PathBuf]) -> Vec<Diagnostic>),
}

pub(crate) enum RsRule {
    File(fn(&RsFile<'_>) -> Vec<Diagnostic>),
    Folder(fn(&Path, &FileSet, &[PathBuf]) -> Vec<Diagnostic>),
}

const FRAMEWORK: Rules = Rules {
    css: &[
        fe::css::rule_empty_line_before::check,
        fe::css::selector_nested_pattern::check,
    ],
    ts: &[
        TsRule::File(fe::ts::no_restricted_imports::check),
        TsRule::File(fe::ts::no_comments::check),
        TsRule::File(fe::ts::no_default_export::check),
        TsRule::File(fe::ts::declaration_order::check),
        TsRule::Folder(fe::folder::index_file::check),
        TsRule::Folder(fe::folder::root_reexport::check),
        TsRule::Folder(fe::folder::test_path::check),
    ],
    rs: &[
        RsRule::File(rs::private_static::check),
        RsRule::File(rs::no_comments::check),
        RsRule::File(rs::super_path::check),
        RsRule::File(rs::test_attr::check),
        RsRule::File(rs::env_name::check),
        RsRule::File(rs::exception_new::check),
        RsRule::File(rs::exception_code::check),
        RsRule::File(rs::mutex_lock::check),
        RsRule::File(rs::source_order::check),
        RsRule::File(rs::impl_order::check),
        RsRule::File(rs::mod_file::check),
        RsRule::File(rs::inherent_impl::check),
        RsRule::File(rs::reexport_mod::check),
        RsRule::File(rs::result_type::check),
        RsRule::Folder(rs::folder::test_tree::check),
    ],
};

const PROJECT: Rules = Rules {
    css: &[
        fe::css::rule_empty_line_before::check,
        fe::css::selector_nested_pattern::check,
    ],
    ts: &[
        TsRule::File(fe::ts::no_restricted_imports::check),
        TsRule::File(fe::ts::no_comments::check),
        TsRule::File(fe::ts::declaration_order::check),
        TsRule::Folder(fe::folder::test_path::check),
    ],
    rs: &[
        RsRule::File(rs::private_static::check),
        RsRule::File(rs::no_comments::check),
        RsRule::File(rs::super_path::check),
        RsRule::File(rs::test_attr::check),
        RsRule::File(rs::exception_new::check),
        RsRule::File(rs::exception_code::check),
        RsRule::File(rs::mutex_lock::check),
        RsRule::File(rs::impl_order::check),
        RsRule::File(rs::inherent_impl::check),
        RsRule::File(rs::reexport_mod::check),
        RsRule::File(rs::result_type::check),
        RsRule::Folder(rs::folder::test_tree::check),
    ],
};

pub(crate) fn rules(ruleset: Ruleset) -> &'static Rules {
    match ruleset {
        Ruleset::Framework => &FRAMEWORK,
        Ruleset::Project => &PROJECT,
    }
}
