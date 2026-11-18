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
