use crate::config::Ruleset;
use crate::rule::css;
use crate::rule::css::CssRule;
use crate::rule::folder;
use crate::rule::folder::FolderRule;
use crate::rule::rs;
use crate::rule::rs::RsRule;
use crate::rule::ts;
use crate::rule::ts::TsRule;

pub(crate) struct Rules {
    pub(crate) css: &'static [CssRule],
    pub(crate) ts: &'static [TsRule],
    pub(crate) rs: &'static [RsRule],
    pub(crate) folder: &'static [FolderRule],
}

const FRAMEWORK: Rules = Rules {
    css: &[
        css::empty_line_before::CSS_RULE,
        css::selector_nested_pattern::CSS_RULE,
    ],
    ts: &[
        ts::no_restricted_imports::TS_RULE,
        ts::no_comments::TS_RULE,
        ts::no_default_export::TS_RULE,
        ts::declaration_order::TS_RULE,
        ts::test_path::TS_RULE,
        ts::import_order::TS_RULE,
    ],
    rs: &[
        rs::private_static::RS_RULE,
        rs::no_comments::RS_RULE,
        rs::super_path::RS_RULE,
        rs::env_name::FRAMEWORK_RS_RULE,
        rs::exception_new::FRAMEWORK_RS_RULE,
        rs::exception_code::RS_RULE,
        rs::mutex_lock::RS_RULE,
        rs::source_order::RS_RULE,
        rs::impl_order::RS_RULE,
        rs::mod_file::RS_RULE,
        rs::inherent_impl::RS_RULE,
        rs::result_type::RS_RULE,
        rs::test_attr::RS_RULE,
        rs::import_order::RS_RULE,
        rs::no_nested_import::RS_RULE,
    ],
    folder: &[
        folder::fe_index_file::FOLDER_RULE,
        folder::fe_root_reexport::FOLDER_RULE,
        folder::fe_test_tree::FOLDER_RULE,
        folder::rs_test_tree::FOLDER_RULE,
    ],
};

const PROJECT: Rules = Rules {
    css: &[
        css::empty_line_before::CSS_RULE,
        css::selector_nested_pattern::CSS_RULE,
    ],
    ts: &[
        ts::no_restricted_imports::TS_RULE,
        ts::no_comments::TS_RULE,
        ts::declaration_order::TS_RULE,
        ts::no_default_export::TS_RULE,
        ts::test_path::TS_RULE,
        ts::import_order::TS_RULE,
    ],
    rs: &[
        rs::env_name::RS_RULE,
        rs::private_static::RS_RULE,
        rs::no_comments::RS_RULE,
        rs::super_path::RS_RULE,
        rs::mod_file::RS_RULE,
        rs::source_order::RS_RULE,
        rs::exception_new::RS_RULE,
        rs::exception_code::RS_RULE,
        rs::mutex_lock::RS_RULE,
        rs::impl_order::RS_RULE,
        rs::inherent_impl::RS_RULE,
        rs::result_type::RS_RULE,
        rs::test_attr::RS_RULE,
        rs::import_order::RS_RULE,
        rs::no_nested_import::RS_RULE,
    ],
    folder: &[
        folder::fe_test_tree::FOLDER_RULE,
        folder::rs_test_tree::FOLDER_RULE,
    ],
};

pub(crate) fn rules(ruleset: Ruleset) -> &'static Rules {
    match ruleset {
        Ruleset::Framework => &FRAMEWORK,
        Ruleset::Project => &PROJECT,
    }
}
