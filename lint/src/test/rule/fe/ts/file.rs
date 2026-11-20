use std::path::Path;

use crate::rule::fe::ts::file::{TsFile, parse};
use crate::rule::fe::ts::no_comments::check;
use crate::rule::report::RuleReport;
use oxc_allocator::Allocator;

pub(crate) fn with_source<T>(path: &Path, source: &str, f: impl FnOnce(&TsFile) -> T) -> T {
    let allocator = Allocator::default();
    let program = parse(&allocator, path, source);
    f(&TsFile::new(path, source, program.as_ref()))
}

#[test]
fn a_parsed_file_has_one_statement() {
    let source = "const a = 1;\n";
    let allocator = Allocator::default();
    let program = parse(&allocator, Path::new("src/a.ts"), source).expect("parsed");
    assert_eq!(program.body.len(), 1);
    with_source(Path::new("src/a.ts"), source, |file| {
        let program = file.program.expect("parsed");
        assert_eq!(program.body.len(), 1);
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
fn a_rust_extension_gives_a_typescript_rule_nothing() {
    let allocator = Allocator::default();
    assert!(parse(&allocator, Path::new("src/a.rs"), "const a = 1;\n").is_none());
    assert!(with_source(Path::new("src/a.rs"), "const a = 1;\n", check).is_empty());
}
