use std::path::{Path, PathBuf};

use crate::files::FileSet;
use crate::rule::folder::fe_test_path::check;

fn tree(paths: &[&str]) -> FileSet {
    FileSet::from_paths(paths.iter().map(PathBuf::from))
}

#[test]
fn a_test_file_in_the_test_folder_passes() {
    let files = tree(&[
        "app/src/a.ts",
        "app/src/view.tsx",
        "app/test/a.test.ts",
        "app/test/view.test.tsx",
    ]);
    let srcs = [PathBuf::from("app/src")];
    assert!(check(Path::new("app/src"), &files, &srcs).is_empty());
}

#[test]
fn a_test_file_in_the_source_folder_fails() {
    let files = tree(&["app/src/http/a.test.ts", "app/src/http/b.test.tsx"]);
    let found = check(
        Path::new("app/src/http"),
        &files,
        &[PathBuf::from("app/src")],
    );
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].path, PathBuf::from("app/src/http/a.test.ts"));
    assert_eq!(found[1].path, PathBuf::from("app/src/http/b.test.tsx"));
    assert_eq!(found[0].rule, "test-path");
    assert_eq!(found[0].problem, "A test file is in the source folder.");
    assert_eq!(found[0].fix, "Move the test to the test folder.");
    assert!(found.iter().all(|item| item.problem == found[0].problem));
}
