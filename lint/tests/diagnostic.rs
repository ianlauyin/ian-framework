use std::path::Path;

use ian_lint::diagnostic::{Diagnostic, Tool, suppress};

fn diagnostic(tool: Tool, line: u32, rule: &'static str) -> Diagnostic {
    Diagnostic::new(
        Path::new("src/a.css"),
        line,
        1,
        tool,
        rule,
        "problem",
        "fix",
    )
}

#[test]
fn diagnostic_has_the_fix_line() {
    let frontend = Diagnostic::new(
        Path::new("src/a.css"),
        2,
        1,
        Tool::CodeStyle,
        "name-case",
        "`.b` is a second name.",
        "Rename it to `b-item`.",
    );
    assert_eq!(
        frontend.to_string(),
        "src/a.css:2:1 code-style/name-case: `.b` is a second name.\n    fix: Rename it to `b-item`."
    );
    let rust = Diagnostic::new(
        Path::new("src/app.rs"),
        2,
        5,
        Tool::RsStyle,
        "no-panic",
        "`.unwrap()` panics.",
        "Return an `Exception`.",
    );
    assert_eq!(
        rust.to_string(),
        "src/app.rs:2:5 rs-style/no-panic: `.unwrap()` panics.\n    fix: Return an `Exception`."
    );
}

#[test]
fn a_frontend_ignore_comment_stops_only_a_frontend_rule() {
    let source = "/* code-style-lint-ignore global-prefix: reason */\n.a {}\n.b {}\n.c {} /* code-style-lint-ignore global-prefix */\n";
    let diagnostics = vec![
        diagnostic(Tool::CodeStyle, 1, "global-prefix"),
        diagnostic(Tool::CodeStyle, 2, "global-prefix"),
        diagnostic(Tool::CodeStyle, 2, "class-name-case"),
        diagnostic(Tool::RsStyle, 2, "global-prefix"),
        diagnostic(Tool::CodeStyle, 3, "global-prefix"),
        diagnostic(Tool::CodeStyle, 4, "global-prefix"),
    ];
    let left: Vec<_> = suppress(source, diagnostics)
        .into_iter()
        .map(|item| (item.tool, item.line, item.rule))
        .collect();
    assert_eq!(
        left,
        [
            (Tool::CodeStyle, 2, "class-name-case"),
            (Tool::RsStyle, 2, "global-prefix"),
            (Tool::CodeStyle, 3, "global-prefix")
        ]
    );
}

#[test]
fn a_rust_ignore_comment_stops_only_a_rust_rule() {
    let source = "// rs-lint-ignore no-panic: reason\nfn a() {}\nfn b() {}\nfn c() {} // rs-lint-ignore no-panic\n";
    let diagnostics = vec![
        diagnostic(Tool::RsStyle, 1, "no-panic"),
        diagnostic(Tool::RsStyle, 2, "no-panic"),
        diagnostic(Tool::CodeStyle, 2, "no-panic"),
        diagnostic(Tool::RsStyle, 3, "no-panic"),
        diagnostic(Tool::RsStyle, 4, "no-panic"),
    ];
    let left: Vec<_> = suppress(source, diagnostics)
        .into_iter()
        .map(|item| (item.tool, item.line, item.rule))
        .collect();
    assert_eq!(
        left,
        [
            (Tool::CodeStyle, 2, "no-panic"),
            (Tool::RsStyle, 3, "no-panic")
        ]
    );
}

#[test]
fn an_offset_inside_a_character_uses_the_start_of_that_character() {
    let source = "a\u{1F600}b\nc";
    let at = |offset| {
        let item = Diagnostic::at_offset(
            Path::new("src/a.ts"),
            source,
            offset,
            Tool::CodeStyle,
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
        Tool::CodeStyle,
        "rule",
        "problem",
        "fix",
    );
    assert_eq!((item.line, item.column), (2, 3));
}

#[test]
fn an_offset_gives_a_one_based_line_and_column_in_characters() {
    let source = "ab\n\u{e9}cd";
    let item = Diagnostic::at_offset(
        Path::new("src/a.ts"),
        source,
        6,
        Tool::CodeStyle,
        "rule",
        "problem",
        "fix",
    );
    assert_eq!((item.line, item.column), (2, 3));
}
