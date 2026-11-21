use std::path::{Path, PathBuf};

use crate::source::with_source;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::rs::test_attr::check;

fn check_source(path: &str, source: &str) -> Vec<Diagnostic> {
    with_source(Path::new(path), source, &[PathBuf::from("api/src")], check)
}

#[test]
fn a_test_in_the_test_folder_passes() {
    let source = "\
#[cfg(test)]
mod only_for_test {
    #[test]
    fn reads() {}

    #[tokio::test]
    async fn waits() {}
}
";
    assert!(check_source("api/src/tests/read.rs", source).is_empty());
    let lib = "\
#[cfg(test)]
mod tests;

fn run() {}
";
    assert!(check_source("api/src/lib.rs", lib).is_empty());
    assert!(check_source("api/src/main.rs", lib).is_empty());
}

#[test]
fn an_attribute_that_does_not_name_a_test_passes() {
    let source = "\
#[cfg(feature = \"test\")]
fn a_feature() {}

#[cfg(not(test))]
fn not_in_a_test() {}

#[cfg(all(unix, not(test)))]
fn not_in_a_test_on_unix() {}

#[derive(Debug)]
struct Name;
";
    assert!(check_source("api/src/read.rs", source).is_empty());
}

#[test]
fn a_test_outside_the_test_folder_fails() {
    let source = "\
#[test]
fn reads() {}

#[tokio::test(flavor = \"multi_thread\")]
async fn waits() {}

#[cfg(test)]
fn hidden() {}

#[cfg(test)]
mod tests {}
";
    let found = check_source("api/src/read.rs", source);
    assert_eq!(found.len(), 4);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].rule, "test-attr");
    assert_eq!(found[0].problem, "A test is outside the `tests` folder.");
    assert_eq!(found[0].fix, "Move the test under the `tests` folder.");
    assert_eq!(found[1].line, 4);
    assert_eq!(found[2].line, 7);
    assert_eq!(found[3].line, 10);
    assert!(found.iter().all(|item| item.problem == found[0].problem));
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}

#[test]
fn a_cfg_that_contains_test_and_a_test_macro_fail() {
    let source = "\
#[cfg(all(test, unix))]
mod extra {}

#[cfg(any(test, feature = \"x\"))]
fn helper() {}

#[rstest::rstest]
fn cases() {}

#[rstest]
fn more_cases() {}

#[test_case(1)]
fn one_case(value: u32) {}
";
    let lines: Vec<_> = check_source("api/src/read.rs", source)
        .into_iter()
        .map(|item| item.line)
        .collect();
    assert_eq!(lines, vec![1, 4, 7, 10, 13]);
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(check_source("api/src/read.rs", "fn (").is_empty());
}
