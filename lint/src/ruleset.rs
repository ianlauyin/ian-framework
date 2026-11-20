use crate::config::Ruleset;
use crate::rule::fe;
use crate::rule::fe::css::CssRule;
use crate::rule::fe::ts::TsRule;
use crate::rule::rs;
use crate::rule::rs::RsRule;

pub(crate) struct Rules {
    pub(crate) css: &'static [CssRule],
    pub(crate) ts: &'static [TsRule],
    pub(crate) rs: &'static [RsRule],
}

const FRAMEWORK: Rules = Rules {
    css: &[
        CssRule::new(
            fe::css::rule_empty_line_before::check,
            Some(fe::css::rule_empty_line_before::edit),
        ),
        CssRule::new(fe::css::selector_nested_pattern::check, None),
    ],
    ts: &[
        TsRule::file(fe::ts::no_restricted_imports::check, None),
        TsRule::file(fe::ts::no_comments::check, None),
        TsRule::file(fe::ts::no_default_export::check, None),
        TsRule::file(
            fe::ts::declaration_order::check,
            Some(fe::ts::declaration_order::edit),
        ),
        TsRule::folder(fe::folder::index_file::check, None),
        TsRule::folder(fe::folder::root_reexport::check, None),
        TsRule::folder(fe::folder::test_path::check, None),
    ],
    rs: &[
        RsRule::file(rs::private_static::check, None),
        RsRule::file(rs::no_comments::check, None),
        RsRule::file(rs::super_path::check, None),
        RsRule::file(rs::test_attr::check, None),
        RsRule::file(rs::env_name::check, None),
        RsRule::file(rs::exception_new::check, None),
        RsRule::file(rs::exception_code::check, None),
        RsRule::file(rs::mutex_lock::check, None),
        RsRule::file(rs::source_order::check, Some(rs::source_order::edit)),
        RsRule::file(rs::impl_order::check, Some(rs::impl_order::edit)),
        RsRule::file(rs::mod_file::check, None),
        RsRule::file(rs::inherent_impl::check, None),
        RsRule::file(rs::reexport_mod::check, None),
        RsRule::file(rs::result_type::check, None),
        RsRule::folder(rs::folder::test_tree::check, None),
    ],
};

const PROJECT: Rules = Rules {
    css: &[
        CssRule::new(
            fe::css::rule_empty_line_before::check,
            Some(fe::css::rule_empty_line_before::edit),
        ),
        CssRule::new(fe::css::selector_nested_pattern::check, None),
    ],
    ts: &[
        TsRule::file(fe::ts::no_restricted_imports::check, None),
        TsRule::file(fe::ts::no_comments::check, None),
        TsRule::file(
            fe::ts::declaration_order::check,
            Some(fe::ts::declaration_order::edit),
        ),
        TsRule::folder(fe::folder::test_path::check, None),
    ],
    rs: &[
        RsRule::file(rs::private_static::check, None),
        RsRule::file(rs::no_comments::check, None),
        RsRule::file(rs::super_path::check, None),
        RsRule::file(rs::test_attr::check, None),
        RsRule::file(rs::exception_new::check, None),
        RsRule::file(rs::exception_code::check, None),
        RsRule::file(rs::mutex_lock::check, None),
        RsRule::file(rs::impl_order::check, Some(rs::impl_order::edit)),
        RsRule::file(rs::inherent_impl::check, None),
        RsRule::file(rs::reexport_mod::check, None),
        RsRule::file(rs::result_type::check, None),
        RsRule::folder(rs::folder::test_tree::check, None),
    ],
};

pub(crate) fn rules(ruleset: Ruleset) -> &'static Rules {
    match ruleset {
        Ruleset::Framework => &FRAMEWORK,
        Ruleset::Project => &PROJECT,
    }
}
