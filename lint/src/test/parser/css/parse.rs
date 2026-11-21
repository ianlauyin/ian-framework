use crate::parser::css::parse::{Item, Kind, is_css_whitespace, parse};

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

fn text<'a>(source: &'a str, item: &Item) -> &'a str {
    &source[item.start as usize..item.end as usize]
}

fn children(item: &Item) -> &[Item] {
    match &item.kind {
        Kind::Rule { children, .. } | Kind::AtRule { children, .. } => children,
        Kind::Comment | Kind::Declaration => &[],
    }
}

#[test]
fn a_stray_close_brace_does_not_hide_the_next_at_rule() {
    let source = "a { color: red; } } @media x { b {} }";
    let items = parse(source);
    assert_eq!(items.len(), 2);
    assert!(matches!(items[0].kind, Kind::Rule { .. }));
    assert!(matches!(items[1].kind, Kind::AtRule { .. }));
    assert_eq!(text(source, &items[1]), "@media x { b {} }");
    assert_eq!(children(&items[1]).len(), 1);
}

#[test]
fn a_stray_close_brace_does_not_hide_the_next_rule() {
    let source = "a { b {} }}\nc {}";
    let items = parse(source);
    assert_eq!(items.len(), 2);
    assert_eq!(text(source, &items[1]), "c {}");
}

#[test]
fn a_brace_in_parentheses_does_not_open_a_block() {
    let source = "x { y: url(a{b) } z {}";
    let items = parse(source);
    assert_eq!(items.len(), 2);
    assert_eq!(text(source, &items[0]), "x { y: url(a{b) }");
    assert_eq!(text(source, &items[1]), "z {}");
    assert_eq!(children(&items[0]).len(), 1);
}

#[test]
fn an_at_rule_statement_is_an_at_rule() {
    for (source, index) in [
        ("@charset \"utf-8\";\na {}", 0),
        ("a {}\n@import \"x.css\";", 1),
        ("@import url(x.css)", 0),
    ] {
        let items = parse(source);
        assert!(
            matches!(items[index].kind, Kind::AtRule { brace: None, .. }),
            "{source}"
        );
    }
}

#[test]
fn an_at_rule_without_a_semicolon_before_the_close_brace_is_an_at_rule() {
    for source in ["a { @apply foo }", "a { @apply foo; }"] {
        let items = parse(source);
        let child = &children(&items[0])[0];
        assert!(
            matches!(child.kind, Kind::AtRule { brace: None, .. }),
            "{source}"
        );
        assert!(text(source, child).starts_with("@apply foo"));
    }
}

#[test]
fn an_unclosed_block_holds_the_rest_of_the_file() {
    let source = "a { color: red";
    let items = parse(source);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].end as usize, source.len());
    assert_eq!(text(source, &children(&items[0])[0]), "color: red");
}

#[test]
fn items_do_not_overlap() {
    for source in [
        "x { y: url(a{b) } z {}",
        "a { color: red; } } @media x { b {} }",
        "a { b { c {} } d {} }\ne {}",
    ] {
        let items = parse(source);
        for pair in items.windows(2) {
            assert!(pair[0].end <= pair[1].start, "{source}");
        }
    }
}

#[test]
fn a_start_is_right_after_crlf_non_ascii_and_form_feed() {
    let source = "/* é */\r\né {}\r\n\u{1f600} {}\x0cb {}";
    let items = parse(source);
    let texts: Vec<&str> = items.iter().map(|item| text(source, item)).collect();
    assert_eq!(texts, ["/* é */", "é {}", "\u{1f600} {}", "b {}"]);
}
