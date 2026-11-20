use crate::config::Ruleset;
use crate::rule::fe::css::rule::CssRule;
use crate::rule::fe::css::rule::FRAMEWORK as CSS_FRAMEWORK;
use crate::rule::fe::css::rule::PROJECT as CSS_PROJECT;
use crate::rule::fe::ts::rule::FRAMEWORK as TS_FRAMEWORK;
use crate::rule::fe::ts::rule::PROJECT as TS_PROJECT;
use crate::rule::fe::ts::rule::TsRule;
use crate::rule::rs::rule::FRAMEWORK as RS_FRAMEWORK;
use crate::rule::rs::rule::PROJECT as RS_PROJECT;
use crate::rule::rs::rule::RsRule;

pub(crate) struct Rules {
    pub(crate) css: &'static [CssRule],
    pub(crate) ts: &'static [TsRule],
    pub(crate) rs: &'static [RsRule],
}

const FRAMEWORK: Rules = Rules {
    css: CSS_FRAMEWORK,
    ts: TS_FRAMEWORK,
    rs: RS_FRAMEWORK,
};

const PROJECT: Rules = Rules {
    css: CSS_PROJECT,
    ts: TS_PROJECT,
    rs: RS_PROJECT,
};

pub(crate) fn rules(ruleset: Ruleset) -> &'static Rules {
    match ruleset {
        Ruleset::Framework => &FRAMEWORK,
        Ruleset::Project => &PROJECT,
    }
}
