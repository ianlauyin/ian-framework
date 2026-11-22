// `{selector}` is not a nested selector.
// Use a class, an id, a `&` suffix, or a `>`, `+`, or `~` combinator.
// Why a lint rule: A nested rule names a child by class or id, extends the parent with `&`, or uses a `>`, `+`, or `~` combinator. `&:where(&)` is not a nested selector.

use crate::file::css::parse::Kind;
use crate::file::css::selector::{is_css_ws_char, selectors};
use crate::file::css::source::CssFile;
use crate::rule::css::CssRule;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;

const RULE: &str = "selector-nested-pattern";

pub(crate) const CSS_RULE: CssRule = CssRule::new(check);

pub fn check(file: &CssFile) -> Vec<Diagnostic> {
    let mut report = RuleReport::new(file.path, file.source, RULE);
    walk(file.items, false, file.source, &mut report);
    report.finish()
}

fn is_nested_selector(selector: &str) -> bool {
    let mut chars = selector.chars();
    match chars.next() {
        Some('>' | '+' | '~') => skip_ws_then_non_ws(chars),
        Some('#' | '.') => chars.next().is_some_and(|char| !is_pattern_ws(char)),
        Some('&') => is_ampersand(&selector['&'.len_utf8()..]),
        _ => false,
    }
}

fn is_ampersand(rest: &str) -> bool {
    if is_where_only_parent(rest) {
        return false;
    }
    let mut chars = rest.chars();
    match chars.next() {
        Some(char) if !is_pattern_ws(char) => true,
        Some(_) => {
            let mut chars = rest.chars().skip_while(|char| is_pattern_ws(*char));
            matches!(chars.next(), Some('>' | '+' | '~')) && skip_ws_then_non_ws(chars)
        }
        None => false,
    }
}

fn is_where_only_parent(rest: &str) -> bool {
    let Some(rest) = rest.strip_prefix(":where(") else {
        return false;
    };
    let rest = rest.trim_start_matches(is_pattern_ws);
    let Some(rest) = rest.strip_prefix('&') else {
        return false;
    };
    rest.trim_start_matches(is_pattern_ws).starts_with(')')
}

fn skip_ws_then_non_ws(chars: impl Iterator<Item = char>) -> bool {
    chars
        .skip_while(|char| is_pattern_ws(*char))
        .next()
        .is_some()
}

fn is_pattern_ws(char: char) -> bool {
    char.is_whitespace()
}

fn walk(
    items: &[crate::file::css::parse::Item],
    parent_is_rule: bool,
    source: &str,
    report: &mut RuleReport<'_>,
) {
    for item in items {
        match &item.kind {
            Kind::Rule {
                selector_end,
                children,
            } => {
                if parent_is_rule {
                    check_selector(source, item.start, *selector_end, report);
                }
                walk(children, true, source, report);
            }
            Kind::AtRule { children, .. } => walk(children, false, source, report),
            Kind::Comment | Kind::Declaration => {}
        }
    }
}

fn check_selector(source: &str, start: u32, selector_end: u32, report: &mut RuleReport<'_>) {
    let raw = &source[start as usize..selector_end as usize];
    let selector = raw.trim_end_matches(is_css_ws_char);
    for (offset, text) in selectors(selector) {
        if is_nested_selector(text) {
            continue;
        }
        report.at_offset(
            start + offset as u32,
            format!("`{text}` is not a nested selector."),
            "Use a class, an id, a `&` suffix, or a `>`, `+`, or `~` combinator.",
        );
    }
}
