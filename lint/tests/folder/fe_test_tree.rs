use std::path::{Path, PathBuf};

use ian_lint::files::FileSet;
use ian_lint::rule::folder::fe_test_tree::check;

fn tree(paths: &[&str]) -> FileSet {
    FileSet::from_paths(paths.iter().map(PathBuf::from))
}

#[test]
fn a_dot_src_path_still_checks_the_test_tree() {
    let files = tree(&["app/src/view.ts", "app/tests/missing.test.ts"]);
    let srcs = [PathBuf::from("./app/src")];
    let found = check(Path::new("app/src"), &files, &srcs);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].problem,
        "The test file has no source file `app/src/missing.ts`."
    );
}

#[test]
fn a_matching_test_tree_passes() {
    let files = tree(&[
        "app/src/a.ts",
        "app/src/sleep.ts",
        "app/src/view/screen.tsx",
        "app/tests/a.test.ts",
        "app/tests/helper.ts",
        "app/tests/view/screen.test.tsx",
    ]);
    let srcs = [PathBuf::from("app/src")];
    assert!(check(Path::new("app/src"), &files, &srcs).is_empty());
}

#[test]
fn a_src_with_no_test_folder_passes() {
    let files = tree(&["app/src/a.ts"]);
    let srcs = [PathBuf::from("app/src")];
    assert!(check(Path::new("app/src"), &files, &srcs).is_empty());
}

#[test]
fn a_test_file_with_no_source_file_fails() {
    let files = tree(&[
        "app/src/view.tsx",
        "app/tests/missing.test.ts",
        "app/tests/view.test.ts",
        "app/tests/view/screen.test.tsx",
    ]);
    let srcs = [PathBuf::from("app/src")];
    let found = check(Path::new("app/src"), &files, &srcs);
    assert_eq!(found.len(), 3);
    assert_eq!(found[0].rule, "test-tree");
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
        "The test file has no source file `app/src/view/screen.tsx`."
    );
    assert_eq!(
        found[2].problem,
        "The test file has no source file `app/src/view.ts`."
    );
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}
