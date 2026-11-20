use std::path::{Path, PathBuf};
use std::{env, fs, process};

use crate::config::Ruleset;
use crate::rule::rs::run::{check, preview, rewrite};

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
    assert!(found.iter().any(|item| item.rule == "source-order"));
}

#[test]
fn check_skips_a_missing_file() {
    assert!(check(Path::new("missing/a.rs"), &[], Ruleset::Framework).is_empty());
}

#[test]
fn preview_reports_a_missing_file() {
    assert!(preview(Path::new("missing/a.rs"), Ruleset::Framework).is_err());
}

#[test]
fn rewrite_writes_a_rust_edit() {
    let path = temp_file("rewrite.rs", OUT_OF_ORDER);
    let failures = rewrite(&path, Ruleset::Framework);
    let text = fs::read_to_string(&path).unwrap();
    fs::remove_file(&path).unwrap();
    assert!(failures.is_empty());
    assert!(text.find("pub fn run()").unwrap() < text.find("fn helper()").unwrap());
}
