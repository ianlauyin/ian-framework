use std::path::{Path, PathBuf};

use crate::files::FileSet;
use crate::rule::fe::folder::index_file::check;

#[test]
fn a_folder_with_an_index_passes() {
    let files = FileSet::from_paths([PathBuf::from("app/src/index.ts")]);
    assert!(check(Path::new("app/src"), &files).is_empty());
}

#[test]
fn a_folder_without_an_index_fails() {
    let files = FileSet::from_paths([PathBuf::from("app/src/app.ts")]);
    let found = check(Path::new("app/src"), &files);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].path, PathBuf::from("app/src"));
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].column, 1);
    assert_eq!(found[0].rule, "folder-index");
    assert_eq!(found[0].problem, "The folder has no `index.ts`.");
    assert_eq!(found[0].fix, "Add an `index.ts`.");
}

#[test]
fn a_folder_with_only_an_index_tsx_fails() {
    let files = FileSet::from_paths([PathBuf::from("app/src/index.tsx")]);
    let found = check(Path::new("app/src"), &files);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].rule, "folder-index");
    assert_eq!(found[0].problem, "The folder has no `index.ts`.");
    assert_eq!(found[0].fix, "Add an `index.ts`.");
}
