use std::path::{Path, PathBuf};

use ian_lint::files::FileSet;
use ian_lint::rule::folder::fe_module_folder::check;

fn srcs() -> Vec<PathBuf> {
    vec![PathBuf::from("web/src")]
}

#[test]
fn a_module_folder_with_the_two_files_passes() {
    let files = FileSet::from_paths([
        PathBuf::from("web/src/module/index.ts"),
        PathBuf::from("web/src/module/account/index.ts"),
        PathBuf::from("web/src/module/account/types.ts"),
    ]);
    assert!(check(Path::new("web/src/module/account"), &files, &srcs()).is_empty());
}

#[test]
fn a_module_folder_with_the_index_passes() {
    let files = FileSet::from_paths([PathBuf::from("web/src/module/index.ts")]);
    assert!(check(Path::new("web/src/module"), &files, &srcs()).is_empty());
}

#[test]
fn a_module_folder_without_the_index_fails() {
    let files = FileSet::from_paths([PathBuf::from("web/src/module/account/index.ts")]);
    let found = check(Path::new("web/src/module"), &files, &srcs());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].path, PathBuf::from("web/src/module"));
    assert_eq!(found[0].rule, "module-shape");
    assert_eq!(found[0].problem, "The `module` folder has no `index.ts`.");
    assert_eq!(found[0].fix, "Add `index.ts` for the module registration.");
}

#[test]
fn a_folder_that_is_not_a_module_folder_passes() {
    let files = FileSet::from_paths([
        PathBuf::from("web/src/module/index.ts"),
        PathBuf::from("web/src/module/account/part/helper.ts"),
        PathBuf::from("web/src/page/login/context.ts"),
    ]);
    for path in ["web/src/module/account/part", "web/src/page/login"] {
        assert!(check(Path::new(path), &files, &srcs()).is_empty());
    }
}

#[test]
fn a_module_folder_without_a_file_fails() {
    let files = FileSet::from_paths([PathBuf::from("web/src/module/account/index.tsx")]);
    let found = check(Path::new("web/src/module/account"), &files, &srcs());
    let problems: Vec<_> = found.iter().map(|item| item.problem.as_str()).collect();
    assert_eq!(
        problems,
        [
            "The module folder has no `index.ts`.",
            "The module folder has no `types.ts`.",
        ]
    );
    assert_eq!(found[0].path, PathBuf::from("web/src/module/account"));
    assert_eq!(found[0].rule, "module-shape");
    assert_eq!(
        found[0].fix,
        "Add `index.ts` for the code of the module and `types.ts` for its types."
    );
}
