// `{selector}` is not a nested selector.
// Use a class, an id, a `&` suffix, or a `>`, `+`, or `~` combinator.
// Why a lint rule: A nested rule names a child by class or id, extends the parent with `&`, or uses a `>`, `+`, or `~` combinator. `&:where(&)` is not a nested selector.

use crate::rule::diagnostic::Diagnostic;
use crate::rule::fe::css::parse::{Kind, is_css_whitespace};
use crate::rule::fe::css::rule::CssRule;
use crate::rule::fe::css::source::CssFile;
use crate::rule::report::RuleReport;

const RULE: &str = "selector-nested-pattern";

pub(crate) const CSS_RULE: CssRule = CssRule::new(check);

pub(crate) fn check(file: &CssFile) -> Vec<Diagnostic> {
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
    items: &[crate::rule::fe::css::parse::Item],
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

fn selectors(selector: &str) -> Vec<(usize, &str)> {
    split_commas(selector)
        .into_iter()
        .map(|(offset, part)| strip(offset, part))
        .collect()
}

fn split_commas(selector: &str) -> Vec<(usize, &str)> {
    let bytes = selector.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut index = 0;
    let mut paren = 0u32;
    let mut bracket = 0u32;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'\\' {
            index += 1;
            if index < bytes.len() {
                index += char_len(selector, index);
            }
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            index = skip_string(selector, index);
            continue;
        }
        if byte == b'/' && index + 1 < bytes.len() && bytes[index + 1] == b'*' {
            index = skip_comment(selector, index);
            continue;
        }
        if paren == 0 && bracket == 0 && byte == b',' {
            parts.push((start, &selector[start..index]));
            index += 1;
            start = index;
            continue;
        }
        match byte {
            b'(' => paren += 1,
            b')' => paren = paren.saturating_sub(1),
            b'[' => bracket += 1,
            b']' => bracket = bracket.saturating_sub(1),
            _ => {}
        }
        index += char_len(selector, index);
    }
    parts.push((start, &selector[start..]));
    parts
}

fn strip(part_start: usize, part: &str) -> (usize, &str) {
    let mut text = part;
    let mut offset = part_start;
    loop {
        let trimmed = text.trim_start_matches(is_css_ws_char);
        offset += text.len() - trimmed.len();
        text = trimmed;
        let Some(rest) = strip_leading_comment(text) else {
            break;
        };
        if rest.len() == text.len() {
            break;
        }
        offset += text.len() - rest.len();
        text = rest;
    }
    loop {
        text = text.trim_end_matches(is_css_ws_char);
        let Some(rest) = strip_trailing_comment(text) else {
            break;
        };
        if rest.len() == text.len() {
            break;
        }
        text = rest;
    }
    (offset, text)
}

fn strip_leading_comment(text: &str) -> Option<&str> {
    if !text.starts_with("/*") {
        return None;
    }
    let bytes = text.as_bytes();
    let mut index = 2;
    while index + 1 < bytes.len() {
        if bytes[index] == b'*' && bytes[index + 1] == b'/' {
            return Some(&text[index + 2..]);
        }
        index += char_len(text, index);
    }
    Some("")
}

fn strip_trailing_comment(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    if !text.ends_with("*/") {
        return None;
    }
    let mut index = 0;
    let mut comment_start = None;
    while index + 1 < bytes.len() {
        if bytes[index] == b'"' || bytes[index] == b'\'' {
            index = skip_string(text, index);
            continue;
        }
        if bytes[index] == b'/' && bytes[index + 1] == b'*' {
            let start = index;
            index = skip_comment(text, index);
            if index == bytes.len() {
                comment_start = Some(start);
                break;
            }
            continue;
        }
        index += char_len(text, index);
    }
    comment_start.map(|start| &text[..start])
}

fn skip_string(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let quote = bytes[start];
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index += 1;
            if index < bytes.len() {
                index += char_len(text, index);
            }
            continue;
        }
        if bytes[index] == quote {
            return index + 1;
        }
        if bytes[index] == b'\n' || bytes[index] == b'\r' {
            return index;
        }
        index += char_len(text, index);
    }
    text.len()
}

fn skip_comment(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut index = start + 2;
    while index + 1 < bytes.len() {
        if bytes[index] == b'*' && bytes[index + 1] == b'/' {
            return index + 2;
        }
        index += char_len(text, index);
    }
    text.len()
}

fn char_len(text: &str, index: usize) -> usize {
    text[index..].chars().next().map_or(1, char::len_utf8)
}

fn is_css_ws_char(char: char) -> bool {
    char.is_ascii() && is_css_whitespace(char as u8)
}
