use std::path::{Path, PathBuf};

use crate::source::with_source;
use ian_lint::rule::rs::config_import::check;

fn srcs() -> Vec<PathBuf> {
    vec![PathBuf::from("api/src")]
}

const SOURCE: &str = "\
use ian_framework_rs::env;
use ian_framework_rs::env::required as read;
use ian_framework_rs::env::*;
use ian_framework_rs::gcp::SecretManager;
use ian_framework_rs::gcp::secret_manager::SecretManager as Secrets;
use ian_framework_rs::{env, exception};
use ian_framework_rs::gcp::{SecretManager, PROJECT_ID_INVALID};

fn load() {
    let _ = ian_framework_rs::env::required(name());
    let _ = gcp::SecretManager::new();
}
";

#[test]
fn env_and_secret_in_config_rs_pass() {
    assert!(with_source(Path::new("api/src/config.rs"), SOURCE, &srcs(), check).is_empty());
}

#[test]
fn env_and_secret_in_a_test_file_pass() {
    for path in ["api/src/tests/app.rs", "api/tests/app.rs"] {
        assert!(with_source(Path::new(path), SOURCE, &srcs(), check).is_empty());
    }
}

#[test]
fn a_file_with_no_env_and_no_secret_passes() {
    let source = "\
use std::env;
use ian_framework_rs::exception::Exception;
use ian_framework_rs::gcp::PROJECT_ID_INVALID;
use ian_framework_rs::gcp::storage::Client;
use crate::config::Config;

fn sample(env: u8) -> u8 {
    let _ = \"ian_framework_rs::env::required\";
    let _ = env::temp_dir();
    env
}
";
    assert!(with_source(Path::new("api/src/app.rs"), source, &srcs(), check).is_empty());
}

#[test]
fn env_and_secret_outside_config_rs_fail() {
    for path in [
        "api/src/app.rs",
        "api/src/config/mod.rs",
        "api/src/db/config.rs",
    ] {
        let found = with_source(Path::new(path), SOURCE, &srcs(), check);
        let lines: Vec<_> = found.iter().map(|item| item.line).collect();
        assert_eq!(lines, [1, 2, 3, 4, 5, 6, 7, 10, 11]);
        assert!(found.iter().all(|item| item.rule == "config-import"));
        assert_eq!(
            found[0].problem,
            "`ian_framework_rs::env` is used outside `config.rs`."
        );
        assert_eq!(
            found[0].fix,
            "Read the value in `config.rs` and pass the value to the caller."
        );
        assert_eq!(
            found[3].problem,
            "`SecretManager` is used outside `config.rs`."
        );
        assert_eq!(
            found[4].problem,
            "`secret_manager` is used outside `config.rs`."
        );
        assert_eq!(found[7].problem, found[0].problem);
        assert_eq!(found[8].problem, found[3].problem);
        assert!(found.iter().all(|item| item.fix == found[0].fix));
    }
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(with_source(Path::new("api/src/app.rs"), "fn (", &srcs(), check).is_empty());
}

#[test]
fn env_and_secret_in_a_macro_call_fail() {
    let source = "\
fn load() {
    assert!(ian_framework_rs::env::required(name()).is_ok());
    let _ = vec![gcp::SecretManager::new()];
}
";
    let found = with_source(Path::new("api/src/app.rs"), source, &srcs(), check);
    let lines: Vec<_> = found.iter().map(|item| item.line).collect();
    assert_eq!(lines, [2, 3]);
}
