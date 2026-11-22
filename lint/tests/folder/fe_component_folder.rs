use std::path::{Path, PathBuf};

use ian_lint::files::FileSet;
use ian_lint::rule::folder::fe_component_folder::check;

fn srcs() -> Vec<PathBuf> {
    vec![PathBuf::from("web/src")]
}

#[test]
fn a_component_folder_with_an_index_passes() {
    let files = FileSet::from_paths([PathBuf::from("web/src/component/Button/index.tsx")]);
    assert!(check(Path::new("web/src/component/Button"), &files, &srcs()).is_empty());
}

#[test]
fn a_folder_that_is_not_a_component_folder_passes() {
    let files = FileSet::from_paths([
        PathBuf::from("web/src/component/Button/asset/icon.ts"),
        PathBuf::from("web/src/page/login/context.ts"),
    ]);
    for path in [
        "web/src/component",
        "web/src/component/Button/asset",
        "web/src/page/login",
    ] {
        assert!(check(Path::new(path), &files, &srcs()).is_empty());
    }
}

#[test]
fn a_component_folder_without_an_index_fails() {
    let files = FileSet::from_paths([PathBuf::from("web/src/component/Button/Button.tsx")]);
    let found = check(Path::new("web/src/component/Button"), &files, &srcs());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].path, PathBuf::from("web/src/component/Button"));
    assert_eq!(found[0].rule, "component-folder");
    assert_eq!(found[0].problem, "The component folder has no `index.tsx`.");
    assert_eq!(
        found[0].fix,
        "Add an `index.tsx` that exports the component."
    );
}
