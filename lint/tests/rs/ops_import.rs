use std::path::{Path, PathBuf};

use crate::source::with_source;
use ian_lint::rule::rs::ops_import::check;

fn srcs() -> Vec<PathBuf> {
    vec![PathBuf::from("api/src")]
}

const SOURCE: &str = "\
use crate::ops::state::OpsState;
use crate::ops;
use crate::{db, ops as admin};

fn run() {
    let _ = crate::ops::router::router();
    assert!(crate::ops::ready());
}
";

#[test]
fn an_ops_path_outside_the_ops_module_fails() {
    for path in ["api/src/router/mod.rs", "api/src/state.rs"] {
        let found = with_source(Path::new(path), SOURCE, &srcs(), check);
        let lines: Vec<_> = found.iter().map(|item| item.line).collect();
        assert_eq!(lines, [1, 2, 3, 6, 7]);
        assert_eq!(found[0].rule, "ops-import");
        assert_eq!(
            found[0].problem,
            "A `crate::ops` path is outside the `ops` module."
        );
        assert_eq!(
            found[0].fix,
            "Move the code into the `ops` module, or move the shared item out of it."
        );
    }
}

#[test]
fn an_ops_path_in_an_allowed_file_passes() {
    for path in [
        "api/src/ops/router/mod.rs",
        "api/src/ops.rs",
        "api/src/main.rs",
        "api/src/tests/ops/state.rs",
    ] {
        assert!(with_source(Path::new(path), SOURCE, &srcs(), check).is_empty());
    }
}

#[test]
fn a_path_that_is_not_the_ops_module_passes() {
    let source = "\
use crate::db::ops;
use crate::options::Options;

fn run() {
    let _ = ops::run();
}
";
    assert!(with_source(Path::new("api/src/state.rs"), source, &srcs(), check).is_empty());
}
