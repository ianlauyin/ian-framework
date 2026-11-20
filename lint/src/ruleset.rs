use crate::config::Ruleset;
use crate::rule::fe;
use crate::rule::fe::css::rule::CssRule;
use crate::rule::fe::ts::rule::TsRule;
use crate::rule::rs;
use crate::rule::rs::rule::RsRule;

pub(crate) struct Rules {
    pub(crate) css: &'static [CssRule],
    pub(crate) ts: &'static [TsRule],
    pub(crate) rs: &'static [RsRule],
}

const FRAMEWORK: Rules = Rules {
    css: &[
        fe::css::file::empty_line_before::CSS_RULE,
        fe::css::file::selector_nested_pattern::CSS_RULE,
    ],
    ts: &[
        fe::ts::file::no_restricted_imports::TS_RULE,
        fe::ts::file::no_comments::TS_RULE,
        fe::ts::file::no_default_export::TS_RULE,
        fe::ts::file::declaration_order::TS_RULE,
        fe::ts::folder::index_file::TS_RULE,
        fe::ts::folder::root_reexport::TS_RULE,
        fe::ts::folder::test_path::TS_RULE,
    ],
    rs: &[
        rs::file::private_static::RS_RULE,
        rs::file::no_comments::RS_RULE,
        rs::file::super_path::RS_RULE,
        rs::file::test_attr::RS_RULE,
        rs::file::env_name::FRAMEWORK_RS_RULE,
        rs::file::exception_new::FRAMEWORK_RS_RULE,
        rs::file::exception_code::RS_RULE,
        rs::file::mutex_lock::RS_RULE,
        rs::file::source_order::RS_RULE,
        rs::file::impl_order::RS_RULE,
        rs::file::mod_file::RS_RULE,
        rs::file::inherent_impl::RS_RULE,
        rs::file::result_type::RS_RULE,
        rs::folder::test_tree::RS_RULE,
    ],
};

const PROJECT: Rules = Rules {
    css: &[
        fe::css::file::empty_line_before::CSS_RULE,
        fe::css::file::selector_nested_pattern::CSS_RULE,
    ],
    ts: &[
        fe::ts::file::no_restricted_imports::TS_RULE,
        fe::ts::file::no_comments::TS_RULE,
        fe::ts::file::declaration_order::TS_RULE,
        fe::ts::file::no_default_export::TS_RULE,
        fe::ts::folder::test_path::TS_RULE,
    ],
    rs: &[
        rs::file::env_name::RS_RULE,
        rs::file::private_static::RS_RULE,
        rs::file::no_comments::RS_RULE,
        rs::file::super_path::RS_RULE,
        rs::file::test_attr::RS_RULE,
        rs::file::mod_file::RS_RULE,
        rs::file::source_order::RS_RULE,
        rs::file::exception_new::RS_RULE,
        rs::file::exception_code::RS_RULE,
        rs::file::mutex_lock::RS_RULE,
        rs::file::impl_order::RS_RULE,
        rs::file::inherent_impl::RS_RULE,
        rs::file::result_type::RS_RULE,
        rs::folder::test_tree::RS_RULE,
    ],
};

pub(crate) fn rules(ruleset: Ruleset) -> &'static Rules {
    match ruleset {
        Ruleset::Framework => &FRAMEWORK,
        Ruleset::Project => &PROJECT,
    }
}
