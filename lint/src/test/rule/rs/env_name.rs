use std::path::{Path, PathBuf};

use crate::rule::rs::env_name::check_source;

fn srcs() -> Vec<PathBuf> {
    vec![PathBuf::from("api/src")]
}

#[test]
fn an_env_read_through_env_rs_passes() {
    let helper = "\
use std::env;

pub fn optional(name: &str) -> Result<String, Exception> {
    env::var(name)
}

pub fn required(name: &str) -> Result<String, Exception> {
    env::var(name)
}

const CODE: &str = \"APP_KEY\";
";
    assert!(check_source(Path::new("api/src/env.rs"), helper, &srcs()).is_empty());
    let gcp = "\
const PROJECT_ENV: &str = \"GOOGLE_CLOUD_PROJECT\";
const CREDENTIALS_ENV: &str = \"GOOGLE_APPLICATION_CREDENTIALS\";

fn read() {
    let _ = env::optional(PROJECT_ENV);
    let _ = env::required(\"GOOGLE_APPLICATION_CREDENTIALS\");
    let _ = env::optional(CREDENTIALS_ENV);
}

fn temp() {
    let _ = std::env::temp_dir();
}

const CODE: &str = \"APP_KEY\";
";
    assert!(check_source(Path::new("api/src/gcp/project.rs"), gcp, &srcs()).is_empty());
    let test = "\
fn read() {
    let _ = std::env::var(\"REDIS_URL\");
    let _ = env!(\"APP_KEY\");
}
";
    assert!(check_source(Path::new("api/src/test/read.rs"), test, &srcs()).is_empty());
}

#[test]
fn an_env_name_outside_gcp_fails() {
    let source = "\
use std::env::var;

const APP_KEY: &str = \"APP_KEY\";

fn read() {
    let _ = std::env::var(\"OTHER\");
    let _ = env::optional(\"APP_PORT\");
    let _ = env::optional(APP_KEY);
    let _ = var(\"DIRECT\");
    let _ = env!(\"BUILT\");
}
";
    let found = check_source(Path::new("api/src/app.rs"), source, &srcs());
    assert_eq!(found.len(), 5);
    assert_eq!(found[0].rule, "env-name");
    assert_eq!(found[0].problem, "An env value is read outside `env.rs`.");
    assert_eq!(found[0].fix, "Read the env value through `env.rs`.");
    assert_eq!(found[1].problem, "The env name `APP_PORT` is not allowed.");
    assert_eq!(
        found[1].fix,
        "Use `GOOGLE_APPLICATION_CREDENTIALS` or `GOOGLE_CLOUD_PROJECT` under `gcp`, or let the caller pass the name."
    );
    assert_eq!(found[2].problem, "The env name `APP_KEY` is not allowed.");
    assert_eq!(found[3].problem, "An env value is read outside `env.rs`.");
    assert_eq!(found[4].problem, "An env value is read outside `env.rs`.");
    assert!(found.iter().all(|item| item.rule == "env-name"));
}

#[test]
fn a_google_env_name_outside_gcp_fails() {
    let source = "\
fn read() {
    let _ = env::optional(\"GOOGLE_CLOUD_PROJECT\");
}
";
    let found = check_source(Path::new("api/src/app.rs"), source, &srcs());
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].problem,
        "The env name `GOOGLE_CLOUD_PROJECT` is not allowed."
    );
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(check_source(Path::new("api/src/app.rs"), "fn (", &srcs()).is_empty());
}
