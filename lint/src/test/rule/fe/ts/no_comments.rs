use std::path::Path;
use std::{env, process};

use crate::diagnostic::Diagnostic;
use crate::rule::fe::ts::file::with_source;
use crate::rule::fe::ts::no_comments::check;

fn diagnostics(name: &str, source: &str) -> Vec<Diagnostic> {
    with_source(Path::new(name), source, check)
}

#[test]
fn a_file_without_a_comment_passes() {
    let source = "\
#!/usr/bin/env node
const value = 1;
";
    assert!(diagnostics("src/a.ts", source).is_empty());
}

#[test]
fn a_comment_fails() {
    let source = "\
#!/usr/bin/env node
// line
/* block */
/** doc */
const value = 1; // trailing
";
    let found = diagnostics("src/a.ts", source);
    assert_eq!(found.len(), 4);
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].column, 1);
    assert_eq!(found[0].rule, "no-comments");
    assert_eq!(found[0].problem, "The file has a comment.");
    assert_eq!(
        found[0].fix,
        "Put the decision in the catalog. Or add a test that prevents that reason."
    );
    assert_eq!(found[1].line, 3);
    assert_eq!(found[1].column, 1);
    assert_eq!(found[2].line, 4);
    assert_eq!(found[2].column, 1);
    assert_eq!(found[3].line, 5);
    assert_eq!(found[3].column, 18);
    assert!(found.iter().all(|item| item.problem == found[0].problem));
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}

#[test]
fn a_jsx_comment_fails() {
    let source = "export const view = <div>{/* note */}</div>;\n";
    let found = diagnostics("src/a.tsx", source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].column, 27);
    assert_eq!(found[0].problem, "The file has a comment.");
}

#[test]
fn a_comment_inside_a_string_passes() {
    let source = "\
const text = \"text://host /* kept */\";
const tick = `// kept /* kept */`;
const pattern = /\\/\\//;
const mark = \"/\";
";
    assert!(diagnostics("src/a.ts", source).is_empty());
}

#[test]
fn a_file_extension_oxc_does_not_accept_has_no_diagnostic() {
    assert!(diagnostics("src/a.rs", "// line\n").is_empty());
}

#[test]
fn a_file_that_cannot_be_read_has_no_diagnostic() {
    let path = env::temp_dir().join(format!("ian-lint-no-comments-ts-missing-{}", process::id()));
    assert!(crate::rule::fe::check(&path, crate::config::Ruleset::Framework).is_empty());
}
