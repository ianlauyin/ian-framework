use std::path::Path;
use std::{env, process};

use crate::rule::diagnostic::Diagnostic;
use crate::rule::fe::css::file::with_source;
use crate::rule::fe::css::selector_nested_pattern::check;

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    with_source(Path::new("src/a.css"), source, check)
}

#[test]
fn a_root_selector_passes() {
    assert!(diagnostics("span {}\n").is_empty());
}

#[test]
fn a_nested_selector_passes() {
    let source = "\
.block {
  .name {}
  #name {}
  &:hover {}
  &.on {}
  &::before {}
  &[hidden] {}
  &:where(.on) {}
  &:where(&, .on) {}
  &:not(&) {}
  > .name {}
  + .name {}
  ~ .name {}
  & > .name {}
  & + .name {}
  & ~ .name {}
  >.name {}
  &  > .name {}
  &:hover,
  &:focus {}
  &:hover, /* keep */ &:focus {}
  &:is(.on, .off) {}
}
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn a_type_selector_fails() {
    let source = "\
.block {
  span {}
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].path, Path::new("src/a.css"));
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].column, 3);
    assert_eq!(found[0].rule, "selector-nested-pattern");
    assert_eq!(found[0].problem, "`span` is not a nested selector.");
    assert_eq!(
        found[0].fix,
        "Use a class, an id, a `&` suffix, or a `>`, `+`, or `~` combinator."
    );
}

#[test]
fn a_descendant_selector_fails() {
    let source = "\
.block {
  & .name {}
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].problem, "`& .name` is not a nested selector.");
}

#[test]
fn a_bare_parent_selector_fails() {
    let source = "\
.block {
  & {}
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].problem, "`&` is not a nested selector.");
}

#[test]
fn a_where_parent_selector_fails() {
    let source = "\
.block {
  &:where(&) {}
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].column, 3);
    assert_eq!(found[0].problem, "`&:where(&)` is not a nested selector.");
}

#[test]
fn a_where_parent_selector_with_spaces_fails() {
    let source = "\
.block {
  &:where( & ) {}
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].problem, "`&:where( & )` is not a nested selector.");
}

#[test]
fn a_pseudo_element_without_a_parent_fails() {
    let source = "\
.block {
  ::before {}
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].problem, "`::before` is not a nested selector.");
}

#[test]
fn one_selector_in_a_list_fails() {
    let source = "\
.block {
  &:hover, span {}
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].column, 12);
    assert_eq!(found[0].problem, "`span` is not a nested selector.");
}

#[test]
fn each_selector_in_a_list_fails() {
    let source = "\
.block {
  span, div {}
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].column, 3);
    assert_eq!(found[0].problem, "`span` is not a nested selector.");
    assert_eq!(found[1].column, 9);
    assert_eq!(found[1].problem, "`div` is not a nested selector.");
}

#[test]
fn a_rule_in_an_at_rule_passes() {
    let source = "\
.block {
  @media (min-width: 1px) {
    span {}
  }
}
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn a_rule_nested_in_a_rule_in_an_at_rule_fails() {
    let source = "\
.block {
  @media (min-width: 1px) {
    .inner {
      span {}
    }
  }
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 4);
    assert_eq!(found[0].column, 7);
    assert_eq!(found[0].problem, "`span` is not a nested selector.");
}

#[test]
fn a_declaration_with_a_string_is_not_a_selector() {
    let source = "\
.a {
  content: \"span {}\";
  span {}
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 3);
    assert_eq!(found[0].problem, "`span` is not a nested selector.");
}

#[test]
fn a_declaration_without_a_semicolon_passes() {
    let source = "\
a {
  color: red
}
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn a_file_that_cannot_be_read_has_no_diagnostic() {
    let path = env::temp_dir().join(format!(
        "ian-lint-selector-nested-missing-{}",
        process::id()
    ));
    assert!(
        crate::rule::fe::check(
            &path.with_extension("css"),
            crate::config::Ruleset::Framework
        )
        .is_empty()
    );
}
