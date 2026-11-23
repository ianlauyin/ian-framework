use std::path::{Path, PathBuf};

use ian_lint::files::FileSet;
use ian_lint::rule::folder::rs_router_folder::check;

fn srcs() -> Vec<PathBuf> {
    vec![PathBuf::from("api/src")]
}

#[test]
fn a_router_with_feature_folders_passes() {
    let files = FileSet::from_paths([
        PathBuf::from("api/src/router/mod.rs"),
        PathBuf::from("api/src/router/routes.rs"),
        PathBuf::from("api/src/router/api/mod.rs"),
        PathBuf::from("api/src/router/api/account/mod.rs"),
        PathBuf::from("api/src/router/api/account/login.rs"),
        PathBuf::from("api/src/router/api/team/member/invite.rs"),
    ]);
    for path in [
        "api/src/router",
        "api/src/router/api",
        "api/src/router/api/account",
        "api/src/router/api/team",
        "api/src/router/api/team/member",
    ] {
        assert!(check(Path::new(path), &files, &srcs()).is_empty());
    }
}

#[test]
fn a_folder_outside_the_api_router_passes() {
    let files = FileSet::from_paths([
        PathBuf::from("api/src/public/page.rs"),
        PathBuf::from("api/src/ops/router/api/public/password.rs"),
        PathBuf::from("api/src/ops/router/api/password.rs"),
    ]);
    for path in [
        "api/src/public",
        "api/src/ops/router/api",
        "api/src/ops/router/api/public",
    ] {
        assert!(check(Path::new(path), &files, &srcs()).is_empty());
    }
}

#[test]
fn a_file_directly_in_the_api_router_fails() {
    let files = FileSet::from_paths([
        PathBuf::from("api/src/router/api/mod.rs"),
        PathBuf::from("api/src/router/api/login.rs"),
        PathBuf::from("api/src/router/api/account/logout.rs"),
    ]);
    let found = check(Path::new("api/src/router/api"), &files, &srcs());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].path, PathBuf::from("api/src/router/api/login.rs"));
    assert_eq!(found[0].rule, "router-folder");
    assert_eq!(
        found[0].problem,
        "The file `login.rs` is directly in `router/api`."
    );
    assert_eq!(found[0].fix, "Move the file to the folder of its feature.");
}

#[test]
fn an_access_tier_folder_fails() {
    let files = FileSet::from_paths([
        PathBuf::from("api/src/router/api/public/account/login.rs"),
        PathBuf::from("api/src/router/api/account/protected/logout.rs"),
    ]);
    for (path, name) in [
        ("api/src/router/api/public", "public"),
        ("api/src/router/api/account/protected", "protected"),
    ] {
        let found = check(Path::new(path), &files, &srcs());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, PathBuf::from(path));
        assert_eq!(
            found[0].problem,
            format!("The folder `{name}` is an access tier.")
        );
        assert_eq!(
            found[0].fix,
            "Give the folder the name of its feature, and put the access tier in the `mod.rs` of the feature."
        );
    }
}
