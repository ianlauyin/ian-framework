use std::path::Path;

use crate::rule::fe::css::file::with_source;
use crate::rule::fe::css::parse::parse;
use crate::rule::report::Report;

#[test]
fn a_parsed_file_has_one_rule() {
    let source = "a {}\n";
    assert_eq!(parse(source).len(), 1);
    with_source(Path::new("src/a.css"), source, |file| {
        assert_eq!(file.items.len(), 1);
        let mut report = Report::new(file.path, file.source, "sample");
        report.at_offset(0, "`a` is wrong.", "Change `a`.");
        let found = report.finish();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, 1);
        assert_eq!(found[0].rule, "sample");
        assert_eq!(found[0].problem, "`a` is wrong.");
    });
}
