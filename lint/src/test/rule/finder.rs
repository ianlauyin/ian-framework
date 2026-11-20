use std::path::Path;

use syn::spanned::Spanned;

use crate::rule::finder::Finder;

#[test]
fn report_records_the_offset_and_a_comment_does_not_stop_it() {
    let source = "let a = 1;\nlet b = 2; // ian-lint-ignore sample\n";
    let mut finder = Finder::new(Path::new("src/a.ts"), source, "sample");
    finder.report(0, "`a` is wrong.", "Change `a`.");
    let offset = source.rfind("let b").unwrap() as u32;
    finder.report(offset, "`b` is wrong.", "Change `b`.");

    let found = finder.finish();
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
    let mut finder = Finder::new(Path::new("src/a.rs"), source, "sample");
    finder.at_span(item.span(), "`b` is wrong.", "Change `b`.");

    let found = finder.finish();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].column, 1);
    assert_eq!(found[0].rule, "sample");
    assert_eq!(found[0].problem, "`b` is wrong.");
}

#[test]
fn rust_source_reports_from_the_closure() {
    let source = "fn a() {}\n";
    let found = Finder::rust_source(Path::new("src/a.rs"), source, "sample", |file, finder| {
        assert_eq!(file.items.len(), 1);
        finder.report(0, "`a` is wrong.", "Change `a`.");
    });
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].rule, "sample");
    assert_eq!(found[0].problem, "`a` is wrong.");
}

#[test]
fn rust_source_has_no_diagnostic_when_the_source_does_not_parse() {
    let found = Finder::rust_source(Path::new("src/a.rs"), "fn {", "sample", |_file, finder| {
        finder.report(0, "`a` is wrong.", "Change `a`.");
    });
    assert!(found.is_empty());
}

#[test]
fn css_source_reports_from_the_closure() {
    let source = "a {}\n";
    let found = Finder::css_source(Path::new("src/a.css"), source, "sample", |items, finder| {
        assert_eq!(items.len(), 1);
        finder.report(0, "`a` is wrong.", "Change `a`.");
    });
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].rule, "sample");
    assert_eq!(found[0].problem, "`a` is wrong.");
}

#[test]
fn typescript_source_reports_from_the_closure() {
    let source = "const a = 1;\n";
    let found = Finder::typescript_source(
        Path::new("src/a.ts"),
        source,
        "sample",
        |program, finder| {
            assert_eq!(program.body.len(), 1);
            finder.report(0, "`a` is wrong.", "Change `a`.");
        },
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].rule, "sample");
    assert_eq!(found[0].problem, "`a` is wrong.");
}

#[test]
fn typescript_source_has_no_diagnostic_when_the_extension_is_not_accepted() {
    let found = Finder::typescript_source(
        Path::new("src/a.rs"),
        "const a = 1;\n",
        "sample",
        |_program, finder| {
            finder.report(0, "`a` is wrong.", "Change `a`.");
        },
    );
    assert!(found.is_empty());
}
