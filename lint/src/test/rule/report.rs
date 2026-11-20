use std::path::Path;

use syn::spanned::Spanned;

use crate::rule::report::RuleReport;

#[test]
fn at_offset_records_the_line_and_column() {
    let source = "let a = 1;\nlet b = 2;\n";
    let mut report = RuleReport::new(Path::new("src/a.ts"), source, "sample");
    report.at_offset(0, "`a` is wrong.", "Change `a`.");
    let offset = source.rfind("let b").unwrap() as u32;
    report.at_offset(offset, "`b` is wrong.", "Change `b`.");

    let found = report.finish();
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].column, 1);
    assert_eq!(found[0].rule, "sample");
    assert_eq!(found[1].line, 2);
    assert_eq!(found[1].rule, "sample");
}

#[test]
fn at_span_reports_the_start_of_the_span() {
    let source = "fn a() {}\nfn b() {}\n";
    let file = syn::parse_file(source).unwrap();
    let syn::Item::Fn(item) = &file.items[1] else {
        panic!("fn");
    };
    let mut report = RuleReport::new(Path::new("src/a.rs"), source, "sample");
    report.at_span(item.span(), "`b` is wrong.", "Change `b`.");

    let found = report.finish();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].column, 1);
    assert_eq!(found[0].rule, "sample");
    assert_eq!(found[0].problem, "`b` is wrong.");
}
