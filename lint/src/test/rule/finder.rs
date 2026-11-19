use std::path::Path;

use crate::rule::finder::Finder;

#[test]
fn report_records_the_offset_and_a_comment_does_not_stop_it() {
    let source = "let a = 1;\nlet b = 2; // ian-lint-ignore sample\n";
    let mut finder = Finder::new(Path::new("src/a.ts"), source);
    finder.report(0, "sample", "`a` is wrong.", "Change `a`.");
    let offset = source.rfind("let b").unwrap() as u32;
    finder.report(offset, "sample", "`b` is wrong.", "Change `b`.");

    let found = finder.finish();
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].column, 1);
    assert_eq!(found[0].rule, "sample");
    assert_eq!(found[1].line, 2);
    assert_eq!(found[1].rule, "sample");
}
