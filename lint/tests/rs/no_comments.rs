use std::path::Path;
use std::{env, process};

use crate::source::with_source;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::rs::no_comments::check;

fn lint(source: &str) -> Vec<Diagnostic> {
    with_source(Path::new("src/lib.rs"), source, &[], check)
}

#[test]
fn a_file_without_a_comment_passes() {
    let source = "\
#!/usr/bin/env sample // kept
fn sample() {
    let value = 1;
}
";
    assert!(lint(source).is_empty());
}

#[test]
fn a_comment_fails() {
    let source = "\
#!/usr/bin/env sample // kept
#![doc(hidden)] // kept
//! module
/// item
fn sample() {
    // line
    /* outer /* inner */ // still */
    /** doc */
    /*! note */
}
";
    let found = lint(source);
    assert_eq!(found.len(), 7);
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].column, 17);
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
    assert_eq!(found[3].line, 6);
    assert_eq!(found[3].column, 5);
    assert_eq!(found[4].line, 7);
    assert_eq!(found[4].column, 5);
    assert_eq!(found[5].line, 8);
    assert_eq!(found[5].column, 5);
    assert_eq!(found[6].line, 9);
    assert_eq!(found[6].column, 5);
    assert!(found.iter().all(|item| item.problem == found[0].problem));
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}

#[test]
fn a_comment_inside_a_string_passes() {
    let source = "\
fn sample() {
    let text = \"text://host /* kept */\";
    let raw = r#\"// kept /* kept */\"#;
    let bytes = b\"// kept\";
    let raw_bytes = br#\"/* kept */\"#;
    let c_text = c\"// kept\";
    let raw_c_text = cr#\"/* kept */\"#;
    let mark = '/';
    let byte_mark = b'/';
}
";
    assert!(lint(source).is_empty());
}

#[test]
fn an_attribute_passes() {
    let source = "\
#![doc(hidden)]
#[doc(hidden)]
#[doc = \"/* kept */ // kept\"]
fn sample() {}
";
    assert!(lint(source).is_empty());
}

#[test]
fn a_comment_in_a_file_that_is_not_valid_rust_fails() {
    let found = lint("fn {\n// note\n");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].column, 1);
    assert_eq!(found[0].rule, "no-comments");
}

#[test]
fn a_file_that_cannot_be_read_is_a_failure() {
    let path = env::temp_dir().join(format!("ian-lint-no-comments-missing-{}", process::id()));
    assert!(
        ian_lint::rule::rs::run::check(&path, &[], ian_lint::config::Ruleset::Framework).is_err()
    );
}
