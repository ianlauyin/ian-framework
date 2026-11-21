use std::path::{Path, PathBuf};
use std::{env, fs, process};

use crate::config::Ruleset;
use crate::rule::rs::run::check;

const OUT_OF_ORDER: &str = "fn helper() {}\n\npub fn run() {}\n";

fn temp_file(name: &str, text: &str) -> PathBuf {
    let path = env::temp_dir().join(format!("ian-lint-rs-run-{}-{name}", process::id()));
    fs::write(&path, text).expect("file is written");
    path
}

#[test]
fn check_reports_a_rust_rule() {
    let path = temp_file("check.rs", OUT_OF_ORDER);
    let found = check(&path, &[], Ruleset::Framework);
    fs::remove_file(&path).unwrap();
    assert!(
        found
            .expect("the file is checked")
            .iter()
            .any(|item| item.rule == "source-order")
    );
}

#[test]
fn a_missing_file_is_a_failure() {
    let output = check(Path::new("missing/a.rs"), &[], Ruleset::Framework).unwrap_err();
    assert!(output.starts_with("missing/a.rs: "));
}

#[test]
fn a_rust_file_that_does_not_parse_is_a_failure() {
    let path = temp_file("broken.rs", "pub fn run() {}\n\nfn helper( {}\n");
    let found = check(&path, &[], Ruleset::Framework);
    fs::remove_file(&path).unwrap();
    let output = found.unwrap_err();
    assert!(output.starts_with(&format!("{}:3:", path.display())));
    assert!(output.contains("The file does not parse"));
}
