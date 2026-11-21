use std::path::PathBuf;
use std::{env, fs, process};

use ian_lint::files::FileSet;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::folder::rs_test_path::check;

fn temp_root(name: &str) -> PathBuf {
    let root = env::temp_dir().join(format!("ian-lint-rs-test-path-{name}-{}", process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old test folder is removed");
    }
    fs::create_dir_all(&root).expect("test folder is created");
    root
}

fn check_source(name: &str, rel: &str, source: &str) -> Vec<Diagnostic> {
    let root = temp_root(name);
    let path = root.join(rel);
    fs::create_dir_all(path.parent().expect("path has a folder")).expect("folder is created");
    fs::write(&path, source).expect("file is written");
    let files = FileSet::from_paths([path.clone()]);
    check(
        path.parent().expect("path has a folder"),
        &files,
        &[root.join("api/src")],
    )
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

#[cfg(feature = \"test\")]
fn not_a_test_attr() {}

#[cfg(any(test, feature = \"x\"))]
fn not_only_test() {}
";
    assert!(check_source("in-folder", "api/src/tests/read.rs", source).is_empty());
    let lib = "\
#[cfg(test)]
mod tests;

fn run() {}
";
    assert!(check_source("lib", "api/src/lib.rs", lib).is_empty());
    assert!(check_source("main", "api/src/main.rs", lib).is_empty());
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
    let found = check_source("outside", "api/src/read.rs", source);
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
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(check_source("invalid", "api/src/read.rs", "fn (").is_empty());
}
