use std::path::{Path, PathBuf};
use std::{env, fs, process};

use crate::config::Side;
use crate::format::format;

fn temp_root(name: &str) -> PathBuf {
    let root = env::temp_dir().join(format!("ian-lint-format-{name}-{}", process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old test folder is removed");
    }
    fs::create_dir_all(&root).expect("test folder is created");
    root
}

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().expect("path has a folder")).expect("folder is created");
    fs::write(path, text).expect("file is written");
}

fn write_config(root: &Path) {
    write(
        root,
        crate::config::FILE_NAME,
        "{\n  \"ruleset\": \"framework\",\n  \"fe\": { \"paths\": [\"web\"] },\n  \"be\": { \"paths\": [\"src\"] }\n}\n",
    );
}

#[test]
fn format_writes_an_edit_and_then_checks_the_root() {
    let root = temp_root("edit");
    write_config(&root);
    write(&root, "web/index.ts", "");
    write(&root, "web/app.css", "a {}\nb {}\n");
    write(&root, "src/app.rs", "fn helper() {}\n\npub fn run() {}\n");

    let report = format(&root, Side::All);
    let css = fs::read_to_string(root.join("web/app.css")).unwrap();
    let rust = fs::read_to_string(root.join("src/app.rs")).unwrap();
    fs::remove_dir_all(&root).unwrap();
    assert!(report.failures.is_empty());
    assert!(report.diagnostics.is_empty());
    assert_eq!(css, "a {}\n\nb {}\n");
    assert!(rust.find("pub fn run()").unwrap() < rust.find("fn helper()").unwrap());
}

#[test]
fn format_writes_only_the_chosen_side() {
    let root = temp_root("side");
    write_config(&root);
    write(&root, "web/index.ts", "");
    write(&root, "web/app.css", "a {}\nb {}\n");
    write(&root, "src/app.rs", "fn helper() {}\n\npub fn run() {}\n");

    let report = format(&root, Side::Rs);
    let css = fs::read_to_string(root.join("web/app.css")).unwrap();
    fs::remove_dir_all(&root).unwrap();
    assert!(report.failures.is_empty());
    assert_eq!(css, "a {}\nb {}\n");
}

#[test]
fn format_reports_a_missing_configuration_file_as_a_failure() {
    let root = temp_root("no-config");
    write(&root, "src/app.rs", "fn helper() {}\n\npub fn run() {}\n");

    let report = format(&root, Side::All);
    let rust = fs::read_to_string(root.join("src/app.rs")).unwrap();
    fs::remove_dir_all(&root).unwrap();
    assert_eq!(report.failures.len(), 1);
    assert_eq!(rust, "fn helper() {}\n\npub fn run() {}\n");
}
