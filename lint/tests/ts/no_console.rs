use std::path::Path;

use crate::source::with_source;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::no_console::check;

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    with_source(Path::new("src/a.ts"), source, check)
}

#[test]
fn a_file_with_no_console_passes() {
    let source = "\
Logger.info(\"app/start\");
const text = \"console.log\";
const item = {console: 1};
item.console;
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn a_use_of_console_fails() {
    let source = "\
console.log(1);
const write = console.error;
window.addEventListener(\"error\", () => console.warn(2));
";
    let found = diagnostics(source);
    let lines: Vec<_> = found.iter().map(|item| item.line).collect();
    assert_eq!(lines, [1, 2, 3]);
    assert_eq!(found[0].rule, "no-console");
    assert_eq!(found[0].problem, "`console` is used.");
    assert_eq!(found[0].fix, "Write the event with `Logger`.");
}
