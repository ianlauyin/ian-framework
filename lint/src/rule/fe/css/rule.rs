use crate::rule::diagnostic::Diagnostic;
use crate::rule::edit::Edit;
use crate::rule::fe::css::file::CssFile;
use crate::rule::fe::css::rule_empty_line_before;
use crate::rule::fe::css::selector_nested_pattern;

pub(crate) struct CssRule {
    pub(crate) check: fn(&CssFile<'_>) -> Vec<Diagnostic>,
    pub(crate) edit: Option<fn(&CssFile<'_>) -> Vec<Edit>>,
}

impl CssRule {
    pub(crate) const fn new(
        check: fn(&CssFile<'_>) -> Vec<Diagnostic>,
        edit: Option<fn(&CssFile<'_>) -> Vec<Edit>>,
    ) -> Self {
        Self { check, edit }
    }
}

pub(crate) const FRAMEWORK: &[CssRule] = &[
    CssRule::new(
        rule_empty_line_before::check,
        Some(rule_empty_line_before::edit),
    ),
    CssRule::new(selector_nested_pattern::check, None),
];

pub(crate) const PROJECT: &[CssRule] = &[
    CssRule::new(
        rule_empty_line_before::check,
        Some(rule_empty_line_before::edit),
    ),
    CssRule::new(selector_nested_pattern::check, None),
];
