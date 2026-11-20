use std::path::{Path, PathBuf};
use std::{env, fs, process};

use crate::config::Ruleset;
use crate::rule::fe::run::check;

fn temp_file(name: &str, text: &str) -> PathBuf {
    let path = env::temp_dir().join(format!("ian-lint-fe-run-{}-{name}", process::id()));
    fs::write(&path, text).expect("file is written");
    path
}

#[test]
fn check_reports_a_css_rule() {
    let path = temp_file("check.css", "a {}\nb {}\n");
    let found = check(&path, Ruleset::Framework);
    fs::remove_file(&path).unwrap();
    assert!(
        found
            .iter()
            .any(|item| item.rule == "rule-empty-line-before")
    );
}

#[test]
fn check_skips_a_missing_file() {
    assert!(check(Path::new("missing/a.ts"), Ruleset::Framework).is_empty());
}
