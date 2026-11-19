use std::path::{Path, PathBuf};

use crate::files::FileSet;
use crate::rule::fe::folder::test_path::check;

fn tree(paths: &[&str]) -> FileSet {
    FileSet::from_paths(paths.iter().map(PathBuf::from))
}

#[test]
fn a_test_file_that_matches_a_source_file_passes() {
    let files = tree(&[
        "app/src/a.ts",
        "app/src/sleep.ts",
        "app/src/view.tsx",
        "app/test/a.test.ts",
        "app/test/view.test.tsx",
        "app/test/helper.ts",
    ]);
    let srcs = [PathBuf::from("app/src")];
    assert!(check(Path::new("app/src"), &files, &srcs).is_empty());
}

#[test]
fn a_test_file_with_no_source_file_fails() {
    let files = tree(&[
        "app/src/view.tsx",
        "app/test/missing.test.ts",
        "app/test/view.test.ts",
    ]);
    let srcs = [PathBuf::from("app/src")];
    let found = check(Path::new("app/src"), &files, &srcs);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].rule, "test-path");
    assert_eq!(
        found[0].problem,
        "The test file has no source file `app/src/missing.ts`."
    );
    assert_eq!(
        found[0].fix,
        "Give the test the same path as the source file."
    );
    assert_eq!(
        found[1].problem,
        "The test file has no source file `app/src/view.ts`."
    );
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}

#[test]
fn a_test_file_in_the_source_folder_fails() {
    let files = tree(&["app/src/http/a.test.ts"]);
    let found = check(
        Path::new("app/src/http"),
        &files,
        &[PathBuf::from("app/src")],
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].path, PathBuf::from("app/src/http/a.test.ts"));
    assert_eq!(found[0].rule, "test-path");
    assert_eq!(found[0].problem, "A test file is in the source folder.");
    assert_eq!(found[0].fix, "Move the test to the test folder.");
}
