use std::path::PathBuf;
use std::{env, fs, process};

use crate::config::{FILE_NAME, Ruleset, load};

fn temp_root(name: &str) -> PathBuf {
    let root = env::temp_dir().join(format!("ian-lint-config-{name}-{}", process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old test folder is removed");
    }
    fs::create_dir_all(&root).expect("test folder is created");
    root
}

fn write_config(root: &PathBuf, text: &str) {
    fs::write(root.join(FILE_NAME), text).expect("config is written");
}

#[test]
fn the_configuration_file_gives_the_ruleset_and_the_src_folders() {
    let root = temp_root("config");
    write_config(
        &root,
        r#"{
            "ruleset": "framework",
            "fe": { "paths": ["./web/src", "admin/src"] },
            "be": { "paths": ["api/src"] }
        }"#,
    );
    let config = load(&root).expect("config loads");
    assert_eq!(config.ruleset, Ruleset::Framework);
    assert_eq!(
        config.fe,
        vec![root.join("web/src"), root.join("admin/src")]
    );
    assert_eq!(config.be, vec![root.join("api/src")]);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_dot_src_path_is_the_root_folder() {
    let root = temp_root("dot");
    write_config(
        &root,
        r#"{"ruleset":"framework","fe":{"paths":["."]},"be":{"paths":[]}}"#,
    );
    let config = load(&root).expect("config loads");
    assert_eq!(config.fe, vec![root.clone()]);
    assert!(config.be.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_missing_configuration_file_is_an_error() {
    let root = temp_root("missing");
    let error = load(&root).expect_err("missing config");
    assert!(error.contains("lint.config.json"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_configuration_file_that_is_not_json_is_an_error() {
    let root = temp_root("bad-json");
    write_config(&root, "{");
    let error = load(&root).expect_err("bad json");
    assert!(error.contains("lint.config.json"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_configuration_file_with_an_unknown_field_is_an_error() {
    let root = temp_root("unknown");
    write_config(
        &root,
        r#"{"ruleset":"framework","fe":{"paths":[]},"be":{"paths":[]},"extra":1}"#,
    );
    let error = load(&root).expect_err("unknown field");
    assert!(error.contains("lint.config.json"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_ruleset_that_is_not_framework_is_an_error() {
    let root = temp_root("ruleset");
    write_config(
        &root,
        r#"{"ruleset":"other","fe":{"paths":[]},"be":{"paths":[]}}"#,
    );
    let error = load(&root).expect_err("ruleset");
    assert!(error.contains("`other` is not a ruleset"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_absolute_src_path_is_an_error() {
    let root = temp_root("absolute");
    write_config(
        &root,
        r#"{"ruleset":"framework","fe":{"paths":["/tmp/src"]},"be":{"paths":[]}}"#,
    );
    let error = load(&root).expect_err("absolute path");
    assert!(error.contains("`/tmp/src` is not a src path"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_empty_src_path_is_an_error() {
    let root = temp_root("empty");
    write_config(
        &root,
        r#"{"ruleset":"framework","fe":{"paths":[""]},"be":{"paths":[]}}"#,
    );
    let error = load(&root).expect_err("empty path");
    assert!(error.contains("is not a src path"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_src_path_that_leaves_the_root_is_an_error() {
    let root = temp_root("parent");
    write_config(
        &root,
        r#"{"ruleset":"framework","fe":{"paths":["web/../api/src"]},"be":{"paths":[]}}"#,
    );
    let error = load(&root).expect_err("parent path");
    assert!(error.contains("`web/../api/src` is not a src path"));
    fs::remove_dir_all(root).unwrap();
}
