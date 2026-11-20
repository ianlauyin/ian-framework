use std::path::Path;

use crate::rule::fe::css::parse::{Kind, is_css_whitespace, parse, read};

#[test]
fn a_rule_holds_its_declarations() {
    let items = parse("a { color: red; }\n");
    assert_eq!(items.len(), 1);
    let Kind::Rule { children, .. } = &items[0].kind else {
        panic!("the item is a rule");
    };
    assert_eq!(children.len(), 1);
    assert!(matches!(children[0].kind, Kind::Declaration));
}

#[test]
fn a_comment_is_an_item() {
    let items = parse("/* a */\na {}\n");
    assert_eq!(items.len(), 2);
    assert!(items[0].is_comment());
    assert!(!items[1].is_comment());
}

#[test]
fn an_at_rule_holds_its_rules() {
    let items = parse("@media (width > 1px) {\n  a {}\n}\n");
    let Kind::AtRule { brace, children } = &items[0].kind else {
        panic!("the item is an at-rule");
    };
    assert!(brace.is_some());
    assert_eq!(children.len(), 1);
}

#[test]
fn a_byte_order_mark_is_not_part_of_the_first_item() {
    let items = parse("\u{feff}a {}\n");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].start as usize, '\u{feff}'.len_utf8());
}

#[test]
fn css_whitespace_is_space_tab_and_line_breaks() {
    assert!(is_css_whitespace(b' '));
    assert!(is_css_whitespace(b'\n'));
    assert!(!is_css_whitespace(b'a'));
}

#[test]
fn a_missing_file_has_no_source() {
    assert!(read(Path::new("missing/a.css")).is_none());
}
