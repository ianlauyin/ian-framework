use std::path::{Path, PathBuf};
use std::{env, fs, process};

use crate::config::Ruleset;
use crate::diagnostic::Diagnostic;
use crate::files::FileSet;
use crate::rule::fe::{check as check_fe_file, check_folder as check_fe_folder};
use crate::rule::rs::check as check_rs_file;

fn rust_file(name: &str, relative: &str, source: &str, ruleset: Ruleset) -> Vec<Diagnostic> {
    let root = env::temp_dir().join(format!("ian-lint-ruleset-rs-{name}-{}", process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old test folder is removed");
    }
    let src = root.join("src");
    let path = src.join(relative);
    fs::create_dir_all(path.parent().expect("path has a folder")).expect("folder is created");
    fs::write(&path, source).expect("file is written");
    let found = check_rs_file(&path, &[src], ruleset);
    fs::remove_dir_all(root).expect("test folder is removed");
    found
}

fn typescript_file(name: &str, source: &str, ruleset: Ruleset) -> Vec<Diagnostic> {
    let root = env::temp_dir().join(format!("ian-lint-ruleset-ts-{name}-{}", process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old test folder is removed");
    }
    fs::create_dir_all(&root).expect("test folder is created");
    let path = root.join("view.ts");
    fs::write(&path, source).expect("file is written");
    let found = check_fe_file(&path, ruleset);
    fs::remove_dir_all(root).expect("test folder is removed");
    found
}

fn temp_root(name: &str) -> PathBuf {
    let root = env::temp_dir().join(format!("ian-lint-ruleset-fe-{name}-{}", process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old test folder is removed");
    }
    fs::create_dir_all(&root).expect("test folder is created");
    root
}

#[test]
fn the_project_ruleset_skips_a_framework_rule() {
    let env_read = "fn read() {\n    let _ = std::env::var(\"APP_PORT\");\n}\n";
    assert!(rust_file("env", "app.rs", env_read, Ruleset::Project).is_empty());
    let order = "use std::fs;\n\nmod child;\n";
    assert!(rust_file("order", "app.rs", order, Ruleset::Project).is_empty());
    assert!(rust_file("mod", "http/mod.rs", "fn routes() {}\n", Ruleset::Project).is_empty());
    let export = "export default function view() {}\n";
    assert!(typescript_file("export", export, Ruleset::Project).is_empty());
    let files = FileSet::from_paths([PathBuf::from("app/src/view.ts")]);
    let srcs = [PathBuf::from("app/src")];
    assert!(check_fe_folder(Path::new("app/src"), &files, &srcs, Ruleset::Project).is_empty());
}

#[test]
fn the_framework_ruleset_reports_a_framework_rule() {
    let env_read = "fn read() {\n    let _ = std::env::var(\"APP_PORT\");\n}\n";
    let env_found = rust_file("env-fw", "app.rs", env_read, Ruleset::Framework);
    assert!(env_found.iter().any(|item| item.rule == "env-name"));
    let order = "use std::fs;\n\nmod child;\n";
    let order_found = rust_file("order-fw", "app.rs", order, Ruleset::Framework);
    assert!(order_found.iter().any(|item| item.rule == "source-order"));
    let mod_found = rust_file(
        "mod-fw",
        "http/mod.rs",
        "fn routes() {}\n",
        Ruleset::Framework,
    );
    assert!(mod_found.iter().any(|item| item.rule == "mod-file"));
    let export = "export default function view() {}\n";
    let export_found = typescript_file("export-fw", export, Ruleset::Framework);
    assert!(
        export_found
            .iter()
            .any(|item| item.rule == "no-default-export")
    );
    let files = FileSet::from_paths([PathBuf::from("app/src/view.ts")]);
    let srcs = [PathBuf::from("app/src")];
    let index_found = check_fe_folder(Path::new("app/src"), &files, &srcs, Ruleset::Framework);
    assert!(index_found.iter().any(|item| item.rule == "folder-index"));
}

#[test]
fn the_project_ruleset_reports_a_shared_rule() {
    let found = rust_file(
        "comment",
        "app.rs",
        "// note\nfn read() {}\n",
        Ruleset::Project,
    );
    assert!(found.iter().any(|item| item.rule == "no-comments"));
}

#[test]
fn the_project_ruleset_skips_a_missing_root_reexport() {
    let root = temp_root("reexport");
    let src = root.join("src");
    fs::create_dir_all(src.join("http")).expect("folder is created");
    fs::write(src.join("index.ts"), "export const local = 1;\n").expect("file is written");
    fs::write(src.join("http/index.ts"), "export const http = 1;\n").expect("file is written");
    let files = FileSet::from_paths([src.join("index.ts"), src.join("http/index.ts")]);
    let found = check_fe_folder(&src, &files, &[src.clone()], Ruleset::Project);
    fs::remove_dir_all(root).expect("test folder is removed");
    assert!(found.iter().all(|item| item.rule != "root-reexport"));
}

#[test]
fn the_framework_ruleset_reports_a_missing_root_reexport() {
    let root = temp_root("reexport-fw");
    let src = root.join("src");
    fs::create_dir_all(src.join("http")).expect("folder is created");
    fs::write(src.join("index.ts"), "export const local = 1;\n").expect("file is written");
    fs::write(src.join("http/index.ts"), "export const http = 1;\n").expect("file is written");
    let files = FileSet::from_paths([src.join("index.ts"), src.join("http/index.ts")]);
    let found = check_fe_folder(&src, &files, &[src.clone()], Ruleset::Framework);
    fs::remove_dir_all(root).expect("test folder is removed");
    assert!(found.iter().any(|item| item.rule == "root-reexport"));
}
