use std::path::{Path, PathBuf};

use crate::parser::rs::source::{RsFile, parse};
use crate::rule::report::RuleReport;
use crate::rule::rs::file::private_static::check;

pub(crate) fn with_source<T>(
    path: &Path,
    source: &str,
    srcs: &[PathBuf],
    f: impl FnOnce(&RsFile) -> T,
) -> T {
    let ast = parse(source);
    f(&RsFile::new(path, source, ast.as_ref(), srcs))
}

#[test]
fn a_parsed_file_has_one_item() {
    let source = "fn a() {}\n";
    let ast = parse(source).expect("parsed");
    assert_eq!(ast.items.len(), 1);
    with_source(Path::new("src/a.rs"), source, &[], |file| {
        let ast = file.ast.expect("parsed");
        assert_eq!(ast.items.len(), 1);
        let mut report = RuleReport::new(file.path, file.source, "sample");
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
