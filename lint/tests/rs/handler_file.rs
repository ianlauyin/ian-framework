use std::path::{Path, PathBuf};

use crate::source::with_source;
use ian_lint::rule::rs::handler_file::check;

fn srcs() -> Vec<PathBuf> {
    vec![PathBuf::from("api/src")]
}

#[test]
fn a_handler_with_the_name_of_its_file_passes() {
    let source = "\
pub(super) struct LoginRequest {}

pub(super) async fn login() {}

async fn load() {}

fn failed() {}
";
    for path in [
        "api/src/router/api/account/login.rs",
        "api/src/ops/router/login.rs",
    ] {
        assert!(with_source(Path::new(path), source, &srcs(), check).is_empty());
    }
}

#[test]
fn a_router_file_with_no_handler_passes() {
    let source = "\
pub(crate) struct AppState {}

pub(crate) fn routes() {}
";
    assert!(
        with_source(
            Path::new("api/src/router/routes.rs"),
            source,
            &srcs(),
            check
        )
        .is_empty()
    );
}

#[test]
fn a_handler_with_a_different_name_or_visibility_fails() {
    let source = "\
pub(super) async fn logout() {}

pub(crate) async fn login() {}
";
    let found = with_source(
        Path::new("api/src/router/account/login.rs"),
        source,
        &srcs(),
        check,
    );
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].rule, "handler-file");
    assert_eq!(
        found[0].problem,
        "The handler `logout` does not have the name of its file."
    );
    assert_eq!(
        found[0].fix,
        "Give the handler the name of the file, or move the handler to its own file."
    );
    assert_eq!(found[1].line, 3);
    assert_eq!(found[1].problem, "The handler `login` is not `pub(super)`.");
    assert_eq!(found[1].fix, "Make the handler `pub(super)`.");
}

#[test]
fn a_handler_in_a_mod_file_fails() {
    let source = "\
pub(super) async fn account() {}

async fn rejected() {}
";
    let found = with_source(
        Path::new("api/src/router/account/mod.rs"),
        source,
        &srcs(),
        check,
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].problem, "The handler `account` is in `mod.rs`.");
    assert_eq!(found[0].fix, "Move the handler to its own file.");
}

#[test]
fn a_file_outside_a_router_folder_passes() {
    let source = "pub(crate) async fn find() {}\n";
    for path in ["api/src/db/account.rs", "api/src/tests/router/login.rs"] {
        assert!(with_source(Path::new(path), source, &srcs(), check).is_empty());
    }
}
