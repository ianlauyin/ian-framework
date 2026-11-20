use crate::rule::fe::css::scan::{Bounds, bounds};

#[test]
fn a_statement_ends_after_its_semicolon() {
    let Bounds::Statement { end } = bounds("a; b", 0, 4) else {
        panic!("the bounds are a statement");
    };
    assert_eq!(end, 2);
}

#[test]
fn a_statement_ends_before_a_close_brace() {
    let Bounds::Statement { end } = bounds("a } b", 0, 5) else {
        panic!("the bounds are a statement");
    };
    assert_eq!(end, 2);
}

#[test]
fn a_close_brace_in_parentheses_does_not_close_the_block() {
    let Bounds::Block {
        brace,
        inner_end,
        end,
    } = bounds("a { (}) } b", 0, 11)
    else {
        panic!("the bounds are a block");
    };
    assert_eq!((brace, inner_end, end), (2, 8, 9));
}
