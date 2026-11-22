use std::path::{Path, PathBuf};

use crate::source::with_source;
use ian_lint::rule::rs::std_env::check;

fn srcs() -> Vec<PathBuf> {
    vec![PathBuf::from("api/src")]
}

const SOURCE: &str = "\
use std::env;
use std::env::var;
use std::env::var as read;
use std::env::*;
use std::{env, fs};
use std::{env::{self, VarError}, fs};
use ::std::env::temp_dir;

fn load() {
    let _ = std::env::var(name());
    let _ = ::std::env::set_current_dir(root());
    let _: std::env::VarError = error();
}
";

#[test]
fn std_env_in_a_project_file_fails() {
    for path in [
        "api/src/app.rs",
        "api/src/config.rs",
        "api/src/env.rs",
        "api/src/main.rs",
    ] {
        let found = with_source(Path::new(path), SOURCE, &srcs(), check);
        let lines: Vec<_> = found.iter().map(|item| item.line).collect();
        assert_eq!(lines, [1, 2, 3, 4, 5, 6, 7, 10, 11, 12]);
        assert!(found.iter().all(|item| item.rule == "std-env"));
        assert!(
            found
                .iter()
                .all(|item| item.problem == "`std::env` is used.")
        );
        assert!(found.iter().all(|item| item.fix
            == "Read the value in `config.rs` with `ian_framework_rs::env` and pass the value to the caller."));
    }
}

#[test]
fn std_env_in_a_test_file_passes() {
    for path in ["api/src/tests/app.rs", "api/tests/app.rs"] {
        assert!(with_source(Path::new(path), SOURCE, &srcs(), check).is_empty());
    }
}

#[test]
fn a_file_with_no_std_env_passes() {
    let source = "\
use std::fs;
use std::path::Path;
use ian_framework_rs::env;
use crate::env::home;

fn sample(env: u8) -> u8 {
    let _ = \"std::env::var\";
    let _ = env::required(name());
    let _ = crate::env::home();
    let _ = other::std::env::var();
    env
}
";
    assert!(with_source(Path::new("api/src/app.rs"), source, &srcs(), check).is_empty());
}

#[test]
fn std_env_in_a_macro_call_fails() {
    let source = "\
fn load() {
    assert!(std::env::var(name()).is_ok());
    let _ = vec![std::env::temp_dir()];
}
";
    let found = with_source(Path::new("api/src/app.rs"), source, &srcs(), check);
    let lines: Vec<_> = found.iter().map(|item| item.line).collect();
    assert_eq!(lines, [2, 3]);
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(with_source(Path::new("api/src/app.rs"), "fn (", &srcs(), check).is_empty());
}
