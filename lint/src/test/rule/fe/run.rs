use std::path::{Path, PathBuf};
use std::{env, fs, process};

use crate::config::Ruleset;
use crate::rule::fe::run::{check, rewrite};

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

#[test]
fn rewrite_writes_a_css_edit() {
    let path = temp_file("rewrite.css", "a {}\nb {}\n");
    let failures = rewrite(&path, Ruleset::Framework);
    let text = fs::read_to_string(&path).unwrap();
    fs::remove_file(&path).unwrap();
    assert!(failures.is_empty());
    assert_eq!(text, "a {}\n\nb {}\n");
}

#[test]
fn rewrite_leaves_a_file_that_passes() {
    let path = temp_file("pass.ts", "const a = 1;\n");
    let failures = rewrite(&path, Ruleset::Framework);
    let text = fs::read_to_string(&path).unwrap();
    fs::remove_file(&path).unwrap();
    assert!(failures.is_empty());
    assert_eq!(text, "const a = 1;\n");
}
