use std::path::Path;

use crate::rule::report::Report;
use crate::rule::rs::file::{parse, with_source};
use crate::rule::rs::private_static::check;

#[test]
fn a_parsed_file_has_one_item() {
    let source = "fn a() {}\n";
    let ast = parse(source).expect("parsed");
    assert_eq!(ast.items.len(), 1);
    with_source(Path::new("src/a.rs"), source, &[], |file| {
        let ast = file.ast.expect("parsed");
        assert_eq!(ast.items.len(), 1);
        let mut report = Report::new(file.path, file.source, "sample");
        report.at_offset(0, "`a` is wrong.", "Change `a`.");
        let found = report.finish();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, 1);
        assert_eq!(found[0].rule, "sample");
        assert_eq!(found[0].problem, "`a` is wrong.");
    });
}

#[test]
fn an_unparsable_source_gives_an_ast_rule_nothing() {
    assert!(parse("fn {").is_none());
    assert!(with_source(Path::new("src/a.rs"), "fn {", &[], check).is_empty());
}
