use std::path::{Path, PathBuf};

use crate::source::with_source;
use ian_lint::rule::rs::no_panic::{check, check_framework};

fn srcs() -> Vec<PathBuf> {
    vec![PathBuf::from("api/src")]
}

const SOURCE: &str = "\
fn run(value: Option<u8>) -> u8 {
    let first = value.unwrap();
    let second = value.expect(\"a value\");
    if first > second {
        panic!(\"order\");
    }
    match first {
        0 => unreachable!(),
        1 => todo!(),
        _ => vec![value.unwrap()][0],
    }
}
";

#[test]
fn a_panic_in_a_source_file_fails() {
    let found = with_source(Path::new("api/src/app.rs"), SOURCE, &srcs(), check);
    let lines: Vec<_> = found.iter().map(|item| item.line).collect();
    assert_eq!(lines, [2, 3, 5, 8, 9, 10]);
    assert_eq!(found[0].rule, "no-panic");
    assert_eq!(found[0].problem, "`unwrap` can stop the process.");
    assert_eq!(found[0].fix, "Return an error.");
    assert_eq!(found[2].problem, "`panic!` can stop the process.");
}

#[test]
fn a_panic_in_a_test_file_passes() {
    for path in ["api/src/tests/app.rs", "api/tests/app.rs"] {
        assert!(with_source(Path::new(path), SOURCE, &srcs(), check).is_empty());
        assert!(with_source(Path::new(path), SOURCE, &srcs(), check_framework).is_empty());
    }
}

#[test]
fn only_the_project_ruleset_allows_main() {
    let path = Path::new("api/src/main.rs");
    assert!(with_source(path, SOURCE, &srcs(), check).is_empty());
    assert_eq!(with_source(path, SOURCE, &srcs(), check_framework).len(), 6);
}

#[test]
fn a_file_with_no_panic_passes() {
    let source = "\
fn run(value: Option<u8>) -> u8 {
    let _ = value.unwrap_or(0);
    let _ = value.unwrap_or_default();
    assert!(value.is_some());
    value.map_or(0, |value| value)
}
";
    assert!(with_source(Path::new("api/src/app.rs"), source, &srcs(), check).is_empty());
}

#[test]
fn a_panic_in_a_const_item_passes() {
    let source = "\
const LIMIT: u8 = match checked() {
    Some(limit) => limit,
    None => panic!(\"the limit is out of range\"),
};

struct App;

impl App {
    const SIZE: u8 = checked().unwrap();
}
";
    assert!(with_source(Path::new("api/src/app.rs"), source, &srcs(), check).is_empty());
}
