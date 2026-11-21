use std::path::{Path, PathBuf};
use std::{env, fs, process};

use crate::config::Ruleset;
use crate::rule::run::check;

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
            .expect("the file is checked")
            .iter()
            .any(|item| item.rule == "rule-empty-line-before")
    );
}

#[test]
fn check_reports_a_typescript_rule() {
    let path = temp_file("check.ts", "export default 1;\n");
    let found = check(&path, Ruleset::Framework);
    fs::remove_file(&path).unwrap();
    assert!(
        found
            .expect("the file is checked")
            .iter()
            .any(|item| item.rule == "no-default-export")
    );
}

#[test]
fn a_missing_file_is_a_failure() {
    let output = check(Path::new("missing/a.ts"), Ruleset::Framework).unwrap_err();
    assert!(output.starts_with("missing/a.ts: "));
}

#[test]
fn a_typescript_file_that_does_not_parse_is_a_failure() {
    let path = temp_file("broken.ts", "export default 1;\nconst a = ;\n");
    let found = check(&path, Ruleset::Framework);
    fs::remove_file(&path).unwrap();
    let output = found.unwrap_err();
    assert!(output.starts_with(&format!("{}:2:", path.display())));
    assert!(output.contains("The file does not parse"));
}
