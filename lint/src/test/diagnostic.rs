use std::path::Path;

use crate::diagnostic::{Diagnostic, suppress};

fn diagnostic(line: u32, rule: &'static str) -> Diagnostic {
    Diagnostic::new(Path::new("src/a.css"), line, 1, rule, "problem", "fix")
}

#[test]
fn diagnostic_has_the_fix_line() {
    let frontend = Diagnostic::new(
        Path::new("src/a.css"),
        2,
        1,
        "name-case",
        "`.b` is a second name.",
        "Rename it to `b-item`.",
    );
    assert_eq!(
        frontend.to_string(),
        "src/a.css:2:1 ian-lint/name-case: `.b` is a second name.\n    fix: Rename it to `b-item`."
    );
    let rust = Diagnostic::new(
        Path::new("src/app.rs"),
        2,
        5,
        "no-panic",
        "`.unwrap()` panics.",
        "Return an `Exception`.",
    );
    assert_eq!(
        rust.to_string(),
        "src/app.rs:2:5 ian-lint/no-panic: `.unwrap()` panics.\n    fix: Return an `Exception`."
    );
}

#[test]
fn an_ignore_comment_stops_that_rule_on_the_same_line_and_the_next_line() {
    let source = "/* ian-lint-ignore global-prefix: reason */\n.a {}\n.b {}\n.c {} /* ian-lint-ignore global-prefix */\n";
    let diagnostics = vec![
        diagnostic(1, "global-prefix"),
        diagnostic(2, "global-prefix"),
        diagnostic(2, "class-name-case"),
        diagnostic(3, "global-prefix"),
        diagnostic(4, "global-prefix"),
    ];
    let left: Vec<_> = suppress(source, diagnostics)
        .into_iter()
        .map(|item| (item.line, item.rule))
        .collect();
    assert_eq!(left, [(2, "class-name-case"), (3, "global-prefix")]);
}

#[test]
fn an_offset_inside_a_character_uses_the_start_of_that_character() {
    let source = "a\u{1F600}b\nc";
    let at = |offset| {
        let item = Diagnostic::at_offset(
            Path::new("src/a.ts"),
            source,
            offset,
            "rule",
            "problem",
            "fix",
        );
        (item.line, item.column)
    };
    assert_eq!(at(2), (1, 2));
    assert_eq!(at(3), (1, 2));
    assert_eq!(at(4), (1, 2));
}

#[test]
fn an_offset_past_the_end_uses_the_end_of_the_source() {
    let item = Diagnostic::at_offset(
        Path::new("src/a.ts"),
        "ab\ncd",
        99,
        "rule",
        "problem",
        "fix",
    );
    assert_eq!((item.line, item.column), (2, 3));
}

#[test]
fn an_offset_gives_a_one_based_line_and_column_in_characters() {
    let source = "ab\n\u{e9}cd";
    let item = Diagnostic::at_offset(Path::new("src/a.ts"), source, 6, "rule", "problem", "fix");
    assert_eq!((item.line, item.column), (2, 3));
}
