use std::path::{Path, PathBuf};

use ian_lint::files::FileSet;
use ian_lint::rule::folder::fe_ajax_tree::check;

const AJAX: &str = "web/src/util/ajax/api";

fn srcs() -> Vec<PathBuf> {
    vec![PathBuf::from("web/src"), PathBuf::from("api/src")]
}

#[test]
fn an_ajax_tree_that_matches_the_router_tree_passes() {
    let files = FileSet::from_paths([
        PathBuf::from("web/src/util/ajax/shared-types.ts"),
        PathBuf::from("web/src/util/ajax/other/call.ts"),
        PathBuf::from("web/src/util/ajax/api/account/login.ts"),
        PathBuf::from("web/src/util/ajax/api/team/member/invite.ts"),
        PathBuf::from("api/src/router/api/mod.rs"),
        PathBuf::from("api/src/router/api/account/mod.rs"),
        PathBuf::from("api/src/router/api/account/login.rs"),
        PathBuf::from("api/src/router/api/account/logout.rs"),
        PathBuf::from("api/src/router/api/team/member/invite.rs"),
    ]);
    for path in [AJAX, "web/src/util/ajax", "web/src/util/ajax/api/account"] {
        assert!(check(Path::new(path), &files, &srcs()).is_empty());
    }
}

#[test]
fn a_project_with_no_api_router_passes() {
    let files = FileSet::from_paths([
        PathBuf::from("web/src/util/ajax/api/account/login.ts"),
        PathBuf::from("api/src/lib.rs"),
    ]);
    assert!(check(Path::new(AJAX), &files, &srcs()).is_empty());
}

#[test]
fn an_ajax_file_without_a_handler_file_fails() {
    let files = FileSet::from_paths([
        PathBuf::from("web/src/util/ajax/api/account/login.ts"),
        PathBuf::from("web/src/util/ajax/api/public/account/logout.ts"),
        PathBuf::from("web/src/util/ajax/api/account/mod.ts"),
        PathBuf::from("api/src/router/api/account/mod.rs"),
        PathBuf::from("api/src/router/api/account/login.rs"),
        PathBuf::from("api/src/router/api/account/logout.rs"),
    ]);
    let found = check(Path::new(AJAX), &files, &srcs());
    let problems: Vec<_> = found
        .iter()
        .map(|item| (item.path.to_str(), item.problem.as_str()))
        .collect();
    assert_eq!(
        problems,
        [
            (
                Some("web/src/util/ajax/api/account/mod.ts"),
                "The ajax file has no handler file `api/src/router/api/account/mod.rs`."
            ),
            (
                Some("web/src/util/ajax/api/public/account/logout.ts"),
                "The ajax file has no handler file `api/src/router/api/public/account/logout.rs`."
            ),
        ]
    );
    assert_eq!(found[0].rule, "ajax-tree");
    assert_eq!(
        found[0].fix,
        "Give the ajax file the same path as the handler file."
    );
}
