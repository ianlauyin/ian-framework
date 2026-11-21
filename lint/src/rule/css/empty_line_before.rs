// The rule has no empty line before it.
// Add an empty line before the rule.
// The rule has an empty line before it.
// Remove the empty line before the rule.
// Why a lint rule: An empty line separates a rule from the node before it. The first child of a block and a rule after a single-line comment stay on the next line.

use crate::file::css::parse::{Item, Kind};
use crate::file::css::source::CssFile;
use crate::rule::css::CssRule;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;

const RULE: &str = "rule-empty-line-before";

pub(crate) const CSS_RULE: CssRule = CssRule::new(check);

pub(crate) fn check(file: &CssFile) -> Vec<Diagnostic> {
    let lines = Lines::new(file.source);
    let mut report = RuleReport::new(file.path, file.source, RULE);
    walk(file.items, None, file.source, &lines, &mut report);
    report.finish()
}

fn walk(
    items: &[Item],
    brace: Option<u32>,
    source: &str,
    lines: &Lines,
    report: &mut RuleReport<'_>,
) {
    for (index, item) in items.iter().enumerate() {
        match &item.kind {
            Kind::Rule {
                selector_end,
                children,
            } => {
                if brace.is_some() || index != 0 {
                    report_rule(item, items, index, brace, source, lines, report);
                }
                walk(children, Some(*selector_end), source, lines, report);
            }
            Kind::AtRule {
                brace: Some(at_brace),
                children,
            } => walk(children, Some(*at_brace), source, lines, report),
            Kind::AtRule { .. } | Kind::Comment | Kind::Declaration => {}
        }
    }
}

fn report_rule(
    item: &Item,
    items: &[Item],
    index: usize,
    brace: Option<u32>,
    source: &str,
    lines: &Lines,
    report: &mut RuleReport<'_>,
) {
    let expect_empty = !is_first_nested(source, items, index, brace, lines)
        && !is_after_single_line_comment(source, items, index, brace, lines);
    let has_empty = has_empty_line(gap(source, items, index, brace));
    if expect_empty == has_empty {
        return;
    }
    if expect_empty {
        report.at_offset(
            item.start,
            "The rule has no empty line before it.",
            "Add an empty line before the rule.",
        );
        return;
    }
    report.at_offset(
        item.start,
        "The rule has an empty line before it.",
        "Remove the empty line before the rule.",
    );
}

fn is_first_nested(
    source: &str,
    items: &[Item],
    index: usize,
    brace: Option<u32>,
    lines: &Lines,
) -> bool {
    let Some(brace) = brace else {
        return false;
    };
    if index == 0 {
        return true;
    }
    let first = &items[0];
    if !first.is_comment() || gap(source, items, 0, Some(brace)).contains('\n') {
        return false;
    }
    let brace_line = lines.line(brace);
    if lines.start(first) != brace_line || lines.end(first) != brace_line {
        return false;
    }
    items[1..index]
        .iter()
        .all(|node| node.is_comment() && lines.end(node) == brace_line)
}

fn is_after_single_line_comment(
    source: &str,
    items: &[Item],
    index: usize,
    brace: Option<u32>,
    lines: &Lines,
) -> bool {
    let Some(previous) = index.checked_sub(1) else {
        return false;
    };
    if !items[previous].is_comment()
        || is_shared_line_comment(source, items, previous, index, brace, lines)
    {
        return false;
    }
    lines.start(&items[previous]) == lines.end(&items[previous])
}

fn is_shared_line_comment(
    source: &str,
    items: &[Item],
    index: usize,
    ignored: usize,
    brace: Option<u32>,
    lines: &Lines,
) -> bool {
    if let Some(previous) = previous_non_shared(items, index, lines) {
        if lines.end(&items[previous]) == lines.start(&items[index]) {
            return true;
        }
    }
    if let Some(next) = next_non_shared(items, index, lines) {
        if next != ignored && lines.end(&items[index]) == lines.start(&items[next]) {
            return true;
        }
    }
    brace.is_some() && index == 0 && !gap(source, items, 0, brace).contains('\n')
}

fn previous_non_shared(items: &[Item], index: usize, lines: &Lines) -> Option<usize> {
    let previous = index.checked_sub(1)?;
    if !items[previous].is_comment() {
        return Some(previous);
    }
    if lines.start(&items[index]) == lines.start(&items[previous]) {
        return previous_non_shared(items, previous, lines);
    }
    if let Some(earlier) = previous.checked_sub(1) {
        if lines.start(&items[previous]) == lines.start(&items[earlier]) {
            return previous_non_shared(items, previous, lines);
        }
    }
    Some(previous)
}

fn next_non_shared(items: &[Item], index: usize, lines: &Lines) -> Option<usize> {
    let next = index + 1;
    if next >= items.len() {
        return None;
    }
    if !items[next].is_comment() {
        return Some(next);
    }
    let next_shares_line = lines.start(&items[index]) == lines.start(&items[next]);
    let next_shares_with_follower = items
        .get(next + 1)
        .is_some_and(|follower| lines.start(&items[next]) == lines.start(follower));
    if next_shares_line || next_shares_with_follower {
        return next_non_shared(items, next, lines);
    }
    Some(next)
}

fn gap<'a>(source: &'a str, items: &[Item], index: usize, brace: Option<u32>) -> &'a str {
    let (from, to) = gap_range(items, index, brace);
    if from > to || to as usize > source.len() {
        return "";
    }
    &source[from as usize..to as usize]
}

fn gap_range(items: &[Item], index: usize, brace: Option<u32>) -> (u32, u32) {
    let from = if index == 0 {
        brace.map_or(0, |offset| offset + 1)
    } else {
        items[index - 1].end
    };
    (from, items[index].start)
}

fn has_empty_line(before: &str) -> bool {
    let bytes = before.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\n' {
            let mut next = index + 1;
            while next < bytes.len() && matches!(bytes[next], b'\r' | b'\t' | b' ') {
                next += 1;
            }
            if next < bytes.len() && bytes[next] == b'\n' {
                return true;
            }
        }
        index += 1;
    }
    false
}

struct Lines {
    newlines: Vec<usize>,
}

impl Lines {
    fn new(source: &str) -> Self {
        Self {
            newlines: source
                .bytes()
                .enumerate()
                .filter(|(_, byte)| *byte == b'\n')
                .map(|(index, _)| index)
                .collect(),
        }
    }

    fn line(&self, offset: u32) -> u32 {
        self.newlines
            .partition_point(|index| *index < offset as usize) as u32
            + 1
    }

    fn start(&self, item: &Item) -> u32 {
        self.line(item.start)
    }

    fn end(&self, item: &Item) -> u32 {
        if item.end == 0 {
            return 1;
        }
        self.line(item.end - 1)
    }
}
