use std::path::Path;
use std::{env, process};

use crate::rule::diagnostic::Diagnostic;
use crate::rule::fe::css::file::empty_line_before::check;
use crate::test::rule::fe::css::source::with_source;

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    with_source(Path::new("src/a.css"), source, check)
}

#[test]
fn one_rule_passes() {
    assert!(diagnostics("a {}\n").is_empty());
}

#[test]
fn a_blank_line_before_the_first_rule_passes() {
    assert!(diagnostics("\n\na {}\n").is_empty());
}

#[test]
fn a_second_rule_with_an_empty_line_passes() {
    let source = "\
a {}

b {}
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn a_second_rule_without_an_empty_line_fails() {
    let source = "\
a {}
b {}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].path, Path::new("src/a.css"));
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].column, 1);
    assert_eq!(found[0].rule, "rule-empty-line-before");
    assert_eq!(found[0].problem, "The rule has no empty line before it.");
    assert_eq!(found[0].fix, "Add an empty line before the rule.");
}

#[test]
fn a_crlf_empty_line_passes() {
    assert!(diagnostics("a {}\r\n\r\nb {}\r\n").is_empty());
}

#[test]
fn a_crlf_line_without_an_empty_line_fails() {
    let found = diagnostics("a {}\r\nb {}\r\n");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].problem, "The rule has no empty line before it.");
}

#[test]
fn the_first_rule_in_a_block_passes() {
    let source = "\
a {
  &:hover {}
}
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn an_empty_line_before_the_first_rule_in_a_block_fails() {
    let source = "\
a {

  &:hover {}
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 3);
    assert_eq!(found[0].column, 3);
    assert_eq!(found[0].problem, "The rule has an empty line before it.");
    assert_eq!(found[0].fix, "Remove the empty line before the rule.");
}

#[test]
fn a_later_rule_in_a_block_with_an_empty_line_passes() {
    let source = "\
a {
  &:hover {}

  &:focus {}
}
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn a_later_rule_in_a_block_without_an_empty_line_fails() {
    let source = "\
a {
  &:hover {}
  &:focus {}
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 3);
    assert_eq!(found[0].column, 3);
    assert_eq!(found[0].problem, "The rule has no empty line before it.");
}

#[test]
fn a_rule_after_a_single_line_comment_passes() {
    let source = "\
/* note */
a {}
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn an_empty_line_after_a_single_line_comment_fails() {
    let source = "\
/* note */

a {}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 3);
    assert_eq!(found[0].problem, "The rule has an empty line before it.");
    assert_eq!(found[0].fix, "Remove the empty line before the rule.");
}

#[test]
fn a_rule_after_a_multi_line_comment_with_an_empty_line_passes() {
    let source = "\
/*
 note
*/

a {}
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn a_rule_after_a_multi_line_comment_without_an_empty_line_fails() {
    let source = "\
/*
 note
*/
a {}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 4);
    assert_eq!(found[0].problem, "The rule has no empty line before it.");
}

#[test]
fn a_comment_on_the_same_line_as_the_rule_passes() {
    assert!(diagnostics("/* note */ a {}\n").is_empty());
}

#[test]
fn a_comment_on_the_same_line_as_the_rule_before_it_fails() {
    let source = "\
a {} /* note */
b {}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].problem, "The rule has no empty line before it.");
}

#[test]
fn a_comment_on_the_brace_line_passes() {
    let source = "\
a { /* note */
  b {}
}
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn an_empty_line_after_a_comment_on_the_brace_line_fails() {
    let source = "\
a { /* note */

  b {}
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 3);
    assert_eq!(found[0].problem, "The rule has an empty line before it.");
}

#[test]
fn two_comments_on_the_brace_line_pass() {
    let source = "\
a { /* one */ /* two */
  b {}
}
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn a_rule_after_a_declaration_without_an_empty_line_fails() {
    let source = "\
a {
  color: red;
  &:hover {}
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 3);
    assert_eq!(found[0].column, 3);
}

#[test]
fn a_rule_after_a_declaration_with_an_empty_line_passes() {
    let source = "\
a {
  color: red;

  &:hover {}
}
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn the_first_rule_in_an_at_rule_passes_and_the_next_rule_fails() {
    let source = "\
@media (min-width: 1px) {
  a {}
  b {}
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 3);
    assert_eq!(found[0].column, 3);
    assert_eq!(found[0].problem, "The rule has no empty line before it.");
}

#[test]
fn an_empty_line_before_the_first_rule_in_an_at_rule_fails() {
    let source = "\
@media (min-width: 1px) {

  a {}
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 3);
    assert_eq!(found[0].problem, "The rule has an empty line before it.");
}

#[test]
fn the_second_keyframe_rule_without_an_empty_line_fails() {
    let source = "\
@keyframes a {
  from {}
  to {}
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 3);
    assert_eq!(found[0].problem, "The rule has no empty line before it.");
}

#[test]
fn a_selector_with_a_brace_in_a_string_keeps_the_next_rule() {
    let source = "\
a[title=\"{\"] {}
b {}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].problem, "The rule has no empty line before it.");
}

#[test]
fn a_file_that_cannot_be_read_has_no_diagnostic() {
    let path = env::temp_dir().join(format!(
        "ian-lint-rule-empty-line-missing-{}",
        process::id()
    ));
    assert!(
        crate::rule::fe::run::check(
            &path.with_extension("css"),
            crate::config::Ruleset::Framework
        )
        .is_empty()
    );
}
